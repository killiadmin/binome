import { ref, onUnmounted } from 'vue';
import { io } from 'socket.io-client';

export function useGameSocket() {
    const socket = ref(null);
    const connected = ref(false);
    const gameCode = ref(null);
    const players = ref([]);
    const gameStatus = ref('waiting');
    const isHost = ref(false);
    const hostId = ref(null);
    const currentTurn = ref(null);
    const myId = ref(null);
    const myCharacter = ref(null);
    const currentQuestion = ref(null);
    const lastAnswer = ref(null);
    const lastGuess = ref(null);
    const error = ref(null);

    const connect = () => {
        socket.value = io(import.meta.env.VITE_SOCKET_URL);

        // ── Connexion ────────────────────────────────────────────────────────────
        socket.value.on('connect', () => {
            connected.value = true;
            console.log('Connecté au serveur:', socket.value.id);
        });

        socket.value.on('disconnect', () => {
            connected.value = false;
            console.log('Déconnecté du serveur');
        });

        socket.value.on('error', (data) => {
            error.value = data.message;
            setTimeout(() => error.value = null, 5000);
        });

        // ── État global de la partie (remplace tous les anciens events) ──────────
        socket.value.on('game-state', (data) => {
            gameCode.value = data.code;
            players.value = data.players;
            gameStatus.value = data.status;
            isHost.value = data.isHost;
            hostId.value = data.host;
            currentTurn.value = data.currentTurn;
            myId.value = data.myId;
            myCharacter.value = data.players.find(p => p.id === data.myId)?.character || null;
        });

        // ── Partie créée (pour récupérer le code immédiatement) ──────────────────
        socket.value.on('game-created', (data) => {
            gameCode.value = data.gameCode;
            players.value = data.game.players;
            gameStatus.value = data.game.status;
            isHost.value = data.game.isHost;
            hostId.value = data.game.host;
            myId.value = data.game.myId;
        });

        // ── Personnage secret assigné à ce joueur ────────────────────────────────
        socket.value.on('your-character', (data) => {
            myCharacter.value = data.character;
        });

        // ── Question posée (broadcast à tous) ────────────────────────────────────
        socket.value.on('new-question', (data) => {
            currentQuestion.value = {
                fromName: data.fromName,
                question: data.question,
                targetName: data.targetName,
                targetId: data.targetId,
            };
        });

        // ── Réponse à la question ─────────────────────────────────────────────────
        socket.value.on('question-answered', (data) => {
            lastAnswer.value = {
                fromName: data.fromName,
                answer: data.answer,
                question: data.question,
            };
            currentQuestion.value = null;
        });

        // ── Résultat d'une tentative de devinette ─────────────────────────────────
        socket.value.on('guess-result', (data) => {
            lastGuess.value = {
                fromName: data.fromName,
                guess: data.guess,
                targetName: data.targetName,
                correct: data.correct,
                revealedCharacter: data.revealedCharacter,
            };
        });

        // ── Changement d'hôte ─────────────────────────────────────────────────────
        socket.value.on('host-changed', (data) => {
            hostId.value = data.newHost;
            isHost.value = socket.value.id === data.newHost;
        });
    };

    // ── Emissions ──────────────────────────────────────────────────────────────

    const createGame = (playerName) => {
        socket.value.emit('create-game', { playerName });
    };

    const joinGame = (code, playerName) => {
        gameCode.value = code;
        socket.value.emit('join-game', { gameCode: code, playerName });
    };

    const assignCharacter = (targetId, character) => {
        socket.value.emit('assign-character', {
            gameCode: gameCode.value,
            targetId,
            character,
        });
    };

    const startGame = () => {
        socket.value.emit('start-game', { gameCode: gameCode.value });
    };

    const sendQuestion = (question) => {
        socket.value.emit('send-question', {
            gameCode: gameCode.value,
            question,
        });
    };

    const sendAnswer = (answer, question) => {
        socket.value.emit('send-answer', {
            gameCode: gameCode.value,
            answer,   // 'oui' | 'non' | 'peutetre'
            question,
        });
    };

    const submitGuess = (guess) => {
        socket.value.emit('submit-guess', {
            gameCode: gameCode.value,
            guess,
        });
    };

    const nextTurn = () => {
        socket.value.emit('next-turn', { gameCode: gameCode.value });
    };

    const resetGame = () => {
        socket.value.emit('reset-game', { gameCode: gameCode.value });
    };

    const disconnect = () => {
        if (socket.value) {
            socket.value.disconnect();
        }
    };

    onUnmounted(() => {
        disconnect();
    });

    return {
        // État
        socket,
        connected,
        gameCode,
        players,
        gameStatus,
        isHost,
        hostId,
        currentTurn,
        myId,
        myCharacter,
        currentQuestion,
        lastAnswer,
        lastGuess,
        error,
        // Actions
        connect,
        createGame,
        joinGame,
        assignCharacter,
        startGame,
        sendQuestion,
        sendAnswer,
        submitGuess,
        nextTurn,
        resetGame,
        disconnect,
    };
}