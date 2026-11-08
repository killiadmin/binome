<?php

namespace App\Services;

use App\Enums\ActionType;
use App\Events\AccusationConfirmed;
use App\Events\ActionPlayed;
use App\Events\AnswerCorrected;
use App\Events\AnswerGiven;
use App\Events\CardDrawFinished;
use App\Events\CardDrawn;
use App\Events\CardWordShared;
use App\Events\GameEnded;
use App\Events\PlayerExcluded;
use App\Events\TurnSkipped;
use App\Enums\GameStatus;
use App\Exceptions\GameRuleException;
use App\Jobs\ExpireCardDraw;
use App\Models\Action;
use App\Models\Game;
use App\Models\Player;
use App\Models\Round;
use Illuminate\Support\Arr;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

class ActionService
{
    // Temps laissé au questionneur pour tirer sa carte ET décider de la montrer.
    public const CARD_DRAW_SECONDS = 30;

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
        $action = DB::transaction(function () use ($round, $player, $question, $targetPlayer) {
            $round = $this->lockRound($round);

            $this->validateTurn($round, $player);
            $this->validateHasNotPlayedThisRound($round, $player);
            $this->validateTarget($round->game, $player, $targetPlayer);

            // Les mots interdits ne sont volontairement PAS contrôlés ici.
            // La règle : ce sont les mots interdits du joueur CIBLÉ qui comptent, et
            // c'est lui seul qui juge si la question en contient un. Il répond alors
            // ce qu'il veut (y compris un mensonge). Le questionneur ne doit jamais
            // apprendre qu'il a fauté — d'où l'absence de tout rejet côté serveur.
            return $this->storeAction(
                round:   $round,
                player:  $player,
                type:    ActionType::Question,
                content: $question,
                isValid: true,
                targetPlayer: $targetPlayer,
            );
        });

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
        $action = DB::transaction(function () use ($round, $player, $targetPlayer, $characterName) {
            $round = $this->lockRound($round);

            $this->validateTurn($round, $player);
            $this->validateHasNotPlayedThisRound($round, $player);
            $this->validateTarget($round->game, $player, $targetPlayer);

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

            return $action->load(['player', 'targetPlayer', 'round']);
        });

        broadcast(new ActionPlayed($action));

        return $action;
    }

    // -------------------------------------------------------------------------
    // LOGIQUE INTERNE
    // -------------------------------------------------------------------------

    /**
     * Verrouille le round le temps de valider et d'enregistrer un coup : deux
     * envois simultanés (double tap, requête rejouée sur un réseau lent)
     * passent l'un après l'autre, et le second voit que le joueur a déjà joué.
     */
    private function lockRound(Round $round): Round
    {
        return Round::whereKey($round->id)->lockForUpdate()->firstOrFail();
    }

    /**
     * Vérifie que c'est bien le tour de ce joueur
     */
    private function validateTurn(Round $round, Player $player): void
    {
        if ($round->game->status !== GameStatus::InProgress) {
            throw new GameRuleException("La partie est terminée.");
        }

        if ($round->current_player_id !== $player->id) {
            throw new GameRuleException("Ce n'est pas ton tour de jouer.");
        }

        if ($round->is_finished) {
            throw new GameRuleException("Ce round est déjà terminé.");
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
            throw new GameRuleException("Tu as déjà joué ce round.");
        }
    }

    /**
     * La cible doit être un autre joueur de la partie, encore en jeu. Le
     * front ne propose déjà que ceux-là, mais un téléphone qui a raté un
     * event (écran verrouillé) peut afficher une liste périmée : accuser un
     * éliminé rapporterait une élimination gratuite.
     */
    private function validateTarget(Game $game, Player $player, Player $target): void
    {
        if ($target->id === $player->id) {
            throw new GameRuleException('Tu ne peux pas te cibler toi-même.');
        }

        $binome = $game->binomes()
            ->whereHas('players', fn ($q) => $q->where('player_id', $target->id))
            ->with('players')
            ->first();

        if (! $binome) {
            throw new GameRuleException("{$target->pseudo} ne joue pas dans cette partie.");
        }

        if ($binome->players->firstWhere('id', $target->id)->pivot->is_eliminated) {
            throw new GameRuleException("{$target->pseudo} est déjà éliminé.");
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
     * Avance au joueur suivant ou crée un nouveau round. `$fromPlayerId` :
     * le joueur dont le tour s'achève (voir RoundService::nextTurn).
     * Rien ne bouge si la partie vient de se terminer : pas de round.started
     * diffusé après game.ended.
     */
    private function advanceRound(Round $round, Game $game, ?int $fromPlayerId = null): void
    {
        if ($game->refresh()->status !== GameStatus::InProgress) {
            return;
        }

        if ($this->roundService->nextTurn($round, $fromPlayerId)) {
            $this->roundService->createRound($game, $round->number + 1);
        }
    }

    /**
     * Clôture la partie
     */
    private function endGame(Game $game, array $winners): void
    {
        $game->update(['status' => GameStatus::Finished]);

        // Le round en cours s'arrête avec la partie (sinon GET /games
        // exposerait encore un tour à jouer).
        $game->rounds()->where('is_finished', false)->update(['is_finished' => true]);

        $winnerIds = collect($winners)->pluck('id')->toArray();

        $stats = $this->scoreService->computeAndStore($game, $winnerIds);

        broadcast(new GameEnded($game, collect($winners), $stats));
    }

    public function playAnswer(Action $action, Player $player, string $answer): Action
    {
        if ($action->target_player_id !== $player->id) {
            throw new GameRuleException("Tu n'es pas le joueur ciblé par cette question.");
        }

        if ($action->type !== ActionType::Question) {
            throw new GameRuleException("Cette action n'est pas une question.");
        }

        $round = $action->round;

        // « Je ne sais pas » : le tour attend que le questionneur tire une carte
        // parmi les mots interdits de la cible (s'il en a).
        $cardCount = $answer === 'dont_know'
            ? count($player->getCharacterInGame($round->game)?->forbidden_words ?? [])
            : 0;

        $this->recordAnswer($action, $answer, $cardCount > 0 ? [
            'card_draw_status'   => 'pending',
            'card_count'         => $cardCount,
            'card_draw_deadline' => now()->addSeconds(self::CARD_DRAW_SECONDS),
        ] : []);

        if ($cardCount > 0) {
            ExpireCardDraw::dispatch($action->id)->delay($action->card_draw_deadline);

            return $action;
        }

        $this->advanceRound($round, $round->game, $action->player_id);

        return $action;
    }

    // -------------------------------------------------------------------------
    // TIRAGE DE CARTE (après un « Je ne sais pas »)
    // -------------------------------------------------------------------------

    /**
     * Le questionneur retourne une carte face cachée : il découvre un mot
     * interdit de la cible, tiré au hasard (le paquet est complet à chaque
     * tirage, un mot peut retomber). Le mot n'est renvoyé QU'À lui, dans la
     * réponse HTTP ; les autres apprennent seulement qu'une carte a été tirée.
     */
    public function drawCard(Action $action, Player $player, int $card): string
    {
        $this->assertCardDrawOwner($action, $player);

        if ($action->card_draw_status !== 'pending') {
            throw new GameRuleException('Tu as déjà tiré ta carte.');
        }

        if ($card < 0 || $card >= $action->card_count) {
            throw new GameRuleException('Cette carte n\'existe pas.');
        }

        if ($action->card_draw_deadline->isPast()) {
            $this->expireCardDraw($action->id);

            throw new GameRuleException('Temps écoulé : la carte est perdue.');
        }

        $game  = $action->round->game;
        $words = $action->targetPlayer->getCharacterInGame($game)?->forbidden_words ?? [];

        if (! $words) {
            throw new GameRuleException("{$action->targetPlayer->pseudo} n'a aucun mot interdit.");
        }

        $word = (string) Arr::random($words);

        $updated = Action::whereKey($action->id)
            ->where('card_draw_status', 'pending')
            ->update(['card_draw_status' => 'drawn', 'drawn_word' => $word]);

        if ($updated === 0) {
            throw new GameRuleException('Tu as déjà tiré ta carte.');
        }

        $action->refresh()->load(['player', 'targetPlayer', 'round']);

        broadcast(new CardDrawn($action, $card));

        return $word;
    }

    /**
     * Le questionneur montre (ou garde pour lui) le mot tiré, puis le tour
     * avance. Montré : envoyé à tous les joueurs et spectateurs, SAUF la cible.
     */
    public function decideCardDraw(Action $action, Player $player, bool $share): void
    {
        $this->assertCardDrawOwner($action, $player);

        if ($action->card_draw_status === 'pending') {
            throw new GameRuleException("Tire d'abord une carte.");
        }

        $updated = Action::whereKey($action->id)
            ->where('card_draw_status', 'drawn')
            ->update(['card_draw_status' => $share ? 'shared' : 'kept']);

        if ($updated === 0) {
            throw new GameRuleException('Ce tirage est déjà terminé.');
        }

        $action->refresh()->load(['player', 'targetPlayer', 'round.game.room.players', 'round.game.binomes.players']);
        $game = $action->round->game;

        if ($share) {
            $recipientIds = $game->binomes->flatMap(fn ($binome) => $binome->players)
                ->merge($game->room?->players ?? [])
                ->pluck('id')
                ->unique()
                ->reject(fn ($id) => in_array($id, [$action->player_id, $action->target_player_id], true))
                ->values()
                ->all();

            if ($recipientIds) {
                broadcast(new CardWordShared($action, $recipientIds));
            }
        }

        $this->finishCardDraw($action);
    }

    /**
     * Délai dépassé (ExpireCardDraw, ou l'hôte qui passe le tour d'un joueur
     * déconnecté) : la carte est perdue, le tour avance. Sans effet si le
     * tirage est déjà clos.
     */
    public function expireCardDraw(int $actionId): void
    {
        $updated = Action::whereKey($actionId)
            ->whereIn('card_draw_status', ['pending', 'drawn'])
            ->update(['card_draw_status' => 'expired']);

        if ($updated === 0) {
            return;
        }

        $this->finishCardDraw(Action::with(['player', 'targetPlayer', 'round'])->findOrFail($actionId));
    }

    private function finishCardDraw(Action $action): void
    {
        broadcast(new CardDrawFinished($action));

        $round = $action->round;
        $this->advanceRound($round, $round->game, $action->player_id);
    }

    private function assertCardDrawOwner(Action $action, Player $player): void
    {
        if ($action->player_id !== $player->id) {
            throw new GameRuleException("Ce tirage revient à {$action->player->pseudo}.");
        }

        if (! $action->card_draw_status) {
            throw new GameRuleException("Aucun tirage n'est prévu pour cette question.");
        }

        if (in_array($action->card_draw_status, ['shared', 'kept', 'expired'], true)) {
            throw new GameRuleException('Ce tirage est déjà terminé.');
        }

        if ($action->round->game->status !== GameStatus::InProgress) {
            throw new GameRuleException('La partie est terminée.');
        }
    }

    public function confirmAccusation(Action $action, Player $player, bool $confirmed): Action
    {
        if ($action->target_player_id !== $player->id) {
            throw new GameRuleException("Tu n'es pas le joueur accusé.");
        }

        if ($action->type !== ActionType::Accusation) {
            throw new GameRuleException("Cette action n'est pas une accusation.");
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
     *  - son tirage de carte           → la carte est perdue, le tour avance ;
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
            throw new GameRuleException('Aucun tour en cours.');
        }

        [$blocker, $reason, $pending] = $this->findBlocker($round);

        if ($reason === 'answer') {
            throw new GameRuleException("{$blocker->pseudo} doit répondre à la question : attends son retour, ou exclus-le de la partie.");
        }

        if ($blocker->id === $host->id) {
            throw new GameRuleException("C'est à toi de jouer : tu ne peux pas passer ton propre tour.");
        }

        $this->assertOffline($game, $blocker);

        broadcast(new TurnSkipped($game, $blocker, $host, $reason, $pending?->id));

        if ($reason === 'accusation') {
            $this->resolveAccusation($pending, $this->accusationMatches($game, $pending));
        } elseif ($reason === 'draw') {
            $this->expireCardDraw($pending->id);
        } else {
            $this->advanceRound($round, $game);
        }

        return ['reason' => $reason, 'player' => $blocker];
    }

    /**
     * L'hôte corrige une réponse oui / non déjà donnée (le joueur s'est ravisé
     * à voix haute). Seul le contenu de la réponse change : le tour n'est pas
     * rejoué. « Je ne sais pas » est exclu, il a déclenché un tirage de carte.
     */
    public function correctAnswer(Game $game, Player $host, Action $action, string $answer): Action
    {
        $this->assertHostOfRunningGame($game, $host, "Seul l'hôte peut corriger une réponse.");

        if ($action->round?->game_id !== $game->id) {
            throw new GameRuleException("Cette question n'appartient pas à la partie.");
        }

        if ($action->type !== ActionType::Question || ! $action->is_valid) {
            throw new GameRuleException("Seule une question valide peut être corrigée.");
        }

        if (! in_array($action->answer, ['yes', 'no'], true) || ! in_array($answer, ['yes', 'no'], true)) {
            throw new GameRuleException('Seules les réponses oui / non peuvent être corrigées.');
        }

        $previous = $action->answer;

        if ($previous === $answer) {
            return $action;
        }

        // On garde la réponse d'ORIGINE du joueur, même après plusieurs
        // corrections ; revenir à celle-ci efface la trace.
        $original = $action->answer_corrected_from ?? $previous;
        $action->update($original === $answer
            ? ['answer' => $answer, 'answer_corrected_from' => null, 'answer_corrected_at' => null]
            : ['answer' => $answer, 'answer_corrected_from' => $original, 'answer_corrected_at' => now()]);
        $action->load(['targetPlayer', 'round']);

        broadcast(new AnswerCorrected($action, $previous, $host));

        return $action;
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
            throw new GameRuleException('Tu ne peux pas t\'exclure toi-même.');
        }

        if (! $game->hasPlayer($target->id)) {
            throw new GameRuleException("{$target->pseudo} ne joue pas dans cette partie.");
        }

        $binome   = $this->binomeService->getBinomeOfPlayer($game, $target);
        $excluded = $binome->players->firstWhere('id', $target->id);

        if ($excluded->pivot->is_eliminated) {
            throw new GameRuleException("{$target->pseudo} est déjà éliminé.");
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

    private function assertHostOfRunningGame(
        Game $game,
        Player $player,
        string $notHostMessage = "Seul l'hôte peut décider pour un joueur déconnecté.",
    ): void
    {
        if ($game->status !== GameStatus::InProgress) {
            throw new GameRuleException("La partie n'est pas en cours.");
        }

        if ($game->isVotingCharacters()) {
            throw new GameRuleException('Les joueurs choisissent encore leurs personnages.');
        }

        if ($game->room?->created_by !== $player->id) {
            throw new GameRuleException($notHostMessage);
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
            throw new GameRuleException("{$player->pseudo} est de nouveau connecté : laisse-lui le temps de jouer.");
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

        // Tirage de carte en cours : c'est le questionneur qu'on attend
        $draw = $pending ? null : $round->actions()
            ->where('player_id', $round->current_player_id)
            ->whereIn('card_draw_status', ['pending', 'drawn'])
            ->with('player')
            ->latest('id')
            ->first();

        if ($draw) {
            return [$draw->player, 'draw', $draw];
        }

        if (! $pending) {
            if ($round->actions()->where('player_id', $round->current_player_id)->exists()) {
                throw new GameRuleException("Personne ne bloque la partie pour l'instant.");
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
    private function recordAnswer(Action $action, string $answer, array $cardDraw = []): void
    {
        $updated = Action::whereKey($action->id)
            ->whereNull('answer')
            ->update(['answer' => $answer] + $cardDraw);

        if ($updated === 0) {
            throw new GameRuleException('Cette question a déjà reçu une réponse.');
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
            throw new GameRuleException('Cette accusation a déjà été confirmée.');
        }

        $action->refresh()->load(['player', 'targetPlayer', 'round']);

        $round = $action->round;
        $game  = $round->game;

        broadcast(new AccusationConfirmed($action));

        if ($correct) {
            $this->handleCorrectAccusation($game, $action->player, $action->targetPlayer);
        }

        // Dans tous les cas on avance le round après confirmation
        $this->advanceRound($round, $game, $action->player_id);
    }
}
