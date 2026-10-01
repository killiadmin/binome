<?php

namespace App\Services;

use App\Enums\ActionType;
use App\Events\AccusationConfirmed;
use App\Events\ActionPlayed;
use App\Events\AnswerGiven;
use App\Events\GameEnded;
use App\Events\PlayerExcluded;
use App\Events\TurnSkipped;
use App\Enums\GameStatus;
use App\Models\Action;
use App\Models\Game;
use App\Models\Player;
use App\Models\Round;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Exception;
use RuntimeException;

class ActionService
{
    public function __construct(
        private readonly RoundService  $roundService,
        private readonly BinomeService $binomeService,
        private readonly ScoreService  $scoreService,
        private readonly PresenceService $presenceService,
    ) {}

    // -------------------------------------------------------------------------
    // QUESTION
    // -------------------------------------------------------------------------

    public function playQuestion(Round $round, Player $player, string $question, Player $targetPlayer): Action
    {
        $this->validateTurn($round, $player);
        $this->validateHasNotPlayedThisRound($round, $player);

        // Les mots interdits ne sont volontairement PAS contrôlés ici.
        // La règle : ce sont les mots interdits du joueur CIBLÉ qui comptent, et
        // c'est lui seul qui juge si la question en contient un. Il répond alors
        // ce qu'il veut (y compris un mensonge). Le questionneur ne doit jamais
        // apprendre qu'il a fauté — d'où l'absence de tout rejet côté serveur.
        $action = $this->storeAction(
            round:   $round,
            player:  $player,
            type:    ActionType::Question,
            content: $question,
            isValid: true,
            targetPlayer: $targetPlayer,
        );

        broadcast(new ActionPlayed($action));

        return $action;
    }

    // -------------------------------------------------------------------------
    // ACCUSATION
    // -------------------------------------------------------------------------

    public function playAccusation(
        Round  $round,
        Player $player,
        Player $targetPlayer,
        string $characterName,
    ): Action {
        $game = $round->game;

        $this->validateTurn($round, $player);
        $this->validateHasNotPlayedThisRound($round, $player);

        // Enregistre l'accusation sans vérifier — on attend la confirmation
        $action = $this->storeAction(
            round:        $round,
            player:       $player,
            type:         ActionType::Accusation,
            content:      $characterName,
            isValid:      true,
            targetPlayer: $targetPlayer,
        );

        // Stocke le nom du personnage accusé
        $action->update(['character_name' => $characterName]);
        $action->load(['player', 'targetPlayer', 'round']);

        broadcast(new ActionPlayed($action));

        return $action;
    }

    // -------------------------------------------------------------------------
    // LOGIQUE INTERNE
    // -------------------------------------------------------------------------

    /**
     * Vérifie que c'est bien le tour de ce joueur
     */
    private function validateTurn(Round $round, Player $player): void
    {
        if ($round->current_player_id !== $player->id) {
            throw new Exception("Ce n'est pas ton tour de jouer.");
        }

        if ($round->is_finished) {
            throw new Exception("Ce round est déjà terminé.");
        }
    }

    /**
     * Vérifie que le joueur n'a pas déjà joué dans ce round
     */
    private function validateHasNotPlayedThisRound(Round $round, Player $player): void
    {
        $alreadyPlayed = $round->actions()
            ->where('player_id', $player->id)
            ->where('is_valid', true)
            ->exists();

        if ($alreadyPlayed) {
            throw new Exception("Tu as déjà joué ce round.");
        }
    }

    /**
     * Persiste l'action en base
     */
    private function storeAction(
        Round      $round,
        Player     $player,
        ActionType $type,
        string     $content,
        bool       $isValid,
        ?Player    $targetPlayer      = null,
        ?bool      $accusationCorrect = null,
    ): Action {
        $action = Action::create([
            'round_id'            => $round->id,
            'player_id'           => $player->id,
            'type'                => $type,
            'content'             => $content,
            'is_valid'            => $isValid,
            'target_player_id'    => $targetPlayer?->id,
            'accusation_correct'  => $accusationCorrect,
        ]);

        $action->load(['player', 'targetPlayer', 'round']);

        return $action;
    }

    /**
     * Une accusation correcte : vérifie si le binome est découvert
     * et si la partie est terminée
     */
    private function handleCorrectAccusation(
        Game   $game,
        Player $accuser,
        Player $target,
    ): void {
        DB::transaction(function () use ($game, $accuser, $target) {

            // Élimine le joueur ciblé (sans toucher au binôme)
            $this->binomeService->eliminatePlayer($game, $target, $accuser);

            // Vérifie si la partie est terminée
            $winners = $this->binomeService->checkGameOver($game);

            if ($winners !== null) {
                $this->endGame($game, $winners);
            }
        });
    }

    /**
     * Avance au joueur suivant ou crée un nouveau round
     */
    private function advanceRound(Round $round, Game $game): void
    {
        $roundFinished = $this->roundService->nextTurn($round);

        if ($roundFinished) {
            $game->refresh();
            if ($game->status === \App\Enums\GameStatus::InProgress) {
                $this->roundService->createRound($game, $round->number + 1);
            }
        }
    }

    /**
     * Clôture la partie
     */
    private function endGame(Game $game, array $winners): void
    {
        $game->update(['status' => \App\Enums\GameStatus::Finished]);

        $winnerIds = collect($winners)->pluck('id')->toArray();

        $stats = $this->scoreService->computeAndStore($game, $winnerIds);

        broadcast(new GameEnded($game, collect($winners), $stats));
    }

    public function playAnswer(Action $action, Player $player, string $answer): Action
    {
        if ($action->target_player_id !== $player->id) {
            throw new RuntimeException("Tu n'es pas le joueur ciblé par cette question.");
        }

        $this->recordAnswer($action, $answer);

        $round = $action->round;
        $this->advanceRound($round, $round->game);

        return $action;
    }

    public function confirmAccusation(Action $action, Player $player, bool $confirmed): Action
    {
        if ($action->target_player_id !== $player->id) {
            throw new RuntimeException("Tu n'es pas le joueur accusé.");
        }

        $this->resolveAccusation($action, $confirmed);

        return $action;
    }

    // -------------------------------------------------------------------------
    // JOUEUR DÉCONNECTÉ (partie en pause, l'hôte décide)
    // -------------------------------------------------------------------------

    /**
     * La partie est « en pause » tant que le joueur qu'elle attend est hors
     * ligne. Seul l'hôte peut la débloquer, et seulement si le serveur
     * confirme via Reverb que ce joueur est bien déconnecté :
     *  - son tour de jouer             → le tour passe, sans action enregistrée ;
     *  - sa confirmation d'accusation  → le serveur tranche en comparant le nom
     *    proposé à son vrai personnage (sinon un accusé n'aurait qu'à se
     *    déconnecter pour s'en tirer).
     * Une question attend TOUJOURS sa réponse : pas de passage possible, il
     * faut attendre le retour du joueur ou l'exclure (excludePlayer).
     *
     * @return array{reason: string, player: Player}
     */
    public function skipBlockedTurn(Game $game, Player $host): array
    {
        $this->assertHostOfRunningGame($game, $host);

        $round = $game->currentRound()->first();

        if (! $round) {
            throw new RuntimeException('Aucun tour en cours.');
        }

        [$blocker, $reason, $pending] = $this->findBlocker($round);

        if ($reason === 'answer') {
            throw new RuntimeException("{$blocker->pseudo} doit répondre à la question : attends son retour, ou exclus-le de la partie.");
        }

        if ($blocker->id === $host->id) {
            throw new RuntimeException("C'est à toi de jouer : tu ne peux pas passer ton propre tour.");
        }

        $this->assertOffline($game, $blocker);

        broadcast(new TurnSkipped($game, $blocker, $host, $reason, $pending?->id));

        if ($reason === 'accusation') {
            $this->resolveAccusation($pending, $this->accusationMatches($game, $pending));
        } else {
            $this->advanceRound($round, $game);
        }

        return ['reason' => $reason, 'player' => $blocker];
    }

    /**
     * L'hôte exclut un joueur déconnecté : lui ET son binôme sont éliminés,
     * l'exclu marque 0 point (ScoreService). Un orphelin exclu n'entraîne
     * personne avec lui.
     *
     * Une question ou accusation en attente qui implique un joueur éliminé
     * est supprimée : si elle venait du joueur courant (toujours en jeu), il
     * rejoue ; si le joueur courant est lui-même éliminé, le tour avance.
     *
     * @return Player|null le partenaire éliminé avec lui
     */
    public function excludePlayer(Game $game, Player $host, Player $target): ?Player
    {
        $this->assertHostOfRunningGame($game, $host);

        if ($target->id === $host->id) {
            throw new RuntimeException('Tu ne peux pas t\'exclure toi-même.');
        }

        if (! $game->hasPlayer($target->id)) {
            throw new RuntimeException("{$target->pseudo} ne joue pas dans cette partie.");
        }

        $binome   = $this->binomeService->getBinomeOfPlayer($game, $target);
        $excluded = $binome->players->firstWhere('id', $target->id);

        if ($excluded->pivot->is_eliminated) {
            throw new RuntimeException("{$target->pseudo} est déjà éliminé.");
        }

        $this->assertOffline($game, $target);

        $partner = $binome->players->first(
            fn ($p) => $p->id !== $target->id && ! $p->pivot->is_eliminated
        );

        return DB::transaction(function () use ($game, $host, $target, $binome, $partner) {
            $round       = $game->currentRound()->first();
            $roundNumber = $round?->number ?? (int) $game->rounds()->max('number');
            $removedIds  = array_values(array_filter([$target->id, $partner?->id]));

            $binome->players()->updateExistingPivot($target->id, [
                'is_eliminated'    => true,
                'is_excluded'      => true,
                'eliminated_round' => $roundNumber,
            ]);

            if ($partner) {
                $binome->players()->updateExistingPivot($partner->id, [
                    'is_eliminated'    => true,
                    'eliminated_round' => $roundNumber,
                ]);
            }

            $cancelled = null;
            $advance   = false;

            if ($round) {
                $pending = $this->pendingActionOf($round);

                if (in_array($round->current_player_id, $removedIds, true)) {
                    $cancelled = $pending;
                    $advance   = true;
                } elseif ($pending && in_array($pending->target_player_id, $removedIds, true)) {
                    $cancelled = $pending;
                }
            }

            $cancelled?->delete();

            broadcast(new PlayerExcluded($game, $target, $partner, $host, $cancelled?->id));

            $winners = $this->binomeService->checkGameOver($game);

            if ($winners !== null) {
                $this->endGame($game, $winners);
            } elseif ($advance) {
                $this->advanceRound($round, $game);
            }

            return $partner;
        });
    }

    private function assertHostOfRunningGame(Game $game, Player $player): void
    {
        if ($game->status !== GameStatus::InProgress) {
            throw new RuntimeException("La partie n'est pas en cours.");
        }

        if ($game->room?->created_by !== $player->id) {
            throw new RuntimeException("Seul l'hôte peut décider pour un joueur déconnecté.");
        }
    }

    /**
     * Reverb confirme-t-il que le joueur est hors ligne ? S'il ne répond pas,
     * on laisse l'hôte décider : mieux vaut débloquer une partie que la geler.
     */
    private function assertOffline(Game $game, Player $player): void
    {
        $online = $this->presenceService->onlinePlayerIds("game.{$game->id}");

        if ($online !== null && in_array($player->id, $online, true)) {
            throw new RuntimeException("{$player->pseudo} est de nouveau connecté : laisse-lui le temps de jouer.");
        }
    }

    /**
     * Qui la partie attend-elle ? Le joueur dont c'est le tour, ou la cible de
     * sa question / accusation encore sans réponse.
     *
     * @return array{0: Player, 1: string, 2: ?Action}
     */
    private function findBlocker(Round $round): array
    {
        $pending = $this->pendingActionOf($round);

        if (! $pending) {
            if ($round->actions()->where('player_id', $round->current_player_id)->exists()) {
                throw new RuntimeException("Personne ne bloque la partie pour l'instant.");
            }

            return [$round->currentPlayer, 'turn', null];
        }

        return [
            $pending->targetPlayer,
            $pending->type === ActionType::Question ? 'answer' : 'accusation',
            $pending,
        ];
    }

    /**
     * Question ou accusation du joueur courant qui attend encore la cible.
     */
    private function pendingActionOf(Round $round): ?Action
    {
        $last = $round->actions()
            ->where('player_id', $round->current_player_id)
            ->with('targetPlayer')
            ->latest('id')
            ->first();

        if (! $last) {
            return null;
        }

        $waiting = $last->type === ActionType::Question
            ? $last->is_valid && $last->answer === null
            : $last->accusation_confirmed === null;

        return $waiting ? $last : null;
    }

    /**
     * Le nom proposé correspond-il au personnage de la cible ? Insensible à la
     * casse, aux accents et aux espaces superflus (« iron  man » = « Iron Man »).
     */
    private function accusationMatches(Game $game, Action $accusation): bool
    {
        $character = $accusation->targetPlayer->getCharacterInGame($game);

        $normalize = fn (?string $name) => Str::of((string) $name)->ascii()->lower()->squish()->value();

        return $character !== null
            && $normalize($character->name) === $normalize($accusation->character_name ?? $accusation->content);
    }

    /**
     * Enregistre la réponse à une question. Mise à jour conditionnelle : si la
     * cible répond au moment même où un autre joueur passe son tour, un seul
     * des deux l'emporte et le round n'avance qu'une fois.
     */
    private function recordAnswer(Action $action, string $answer): void
    {
        $updated = Action::whereKey($action->id)
            ->whereNull('answer')
            ->update(['answer' => $answer]);

        if ($updated === 0) {
            throw new RuntimeException('Cette question a déjà reçu une réponse.');
        }

        $action->refresh()->load(['player', 'targetPlayer', 'round']);

        broadcast(new AnswerGiven($action));
    }

    /**
     * Tranche une accusation (confirmée par la cible, ou par le serveur si la
     * cible est hors ligne), puis avance le round. Même garde que recordAnswer().
     */
    private function resolveAccusation(Action $action, bool $correct): void
    {
        $updated = Action::whereKey($action->id)
            ->whereNull('accusation_confirmed')
            ->update([
                'accusation_confirmed' => $correct,
                'accusation_correct'   => $correct,
            ]);

        if ($updated === 0) {
            throw new RuntimeException('Cette accusation a déjà été confirmée.');
        }

        $action->refresh()->load(['player', 'targetPlayer', 'round']);

        $round = $action->round;
        $game  = $round->game;

        broadcast(new AccusationConfirmed($action));

        if ($correct) {
            $this->handleCorrectAccusation($game, $action->player, $action->targetPlayer);
        }

        // Dans tous les cas on avance le round après confirmation
        $this->advanceRound($round, $game);
    }
}
