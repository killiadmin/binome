import { ref, onUnmounted } from 'vue';
import { io } from 'socket.io-client';

export function useGameSocket() {
    const socket = ref(null);
    const connected = ref(false);
    const gameCode = ref(null);
    const players = ref([]);
    const currentQuestion = ref(null);
    const gameStatus = ref('waiting');
    const isHost = ref(false);
    const error = ref(null);

    const connect = () => {
        socket.value = io('http://localhost:3000');

        socket.value.on('connect', () => {
            connected.value = true;
            console.log('Connecté au serveur');
        });

        socket.value.on('disconnect', () => {
            connected.value = false;
            console.log('Déconnecté du serveur');
        });

        socket.value.on('error', (data) => {
            error.value = data.message;
            setTimeout(() => error.value = null, 5000);
        });

        socket.value.on('game-created', (data) => {
            gameCode.value = data.gameCode;
            players.value = data.game.players;
            isHost.value = true;
        });

        socket.value.on('player-joined', (data) => {
            players.value = data.game.players;
        });

        socket.value.on('player-left', (data) => {
            players.value = data.game.players;
        });

        socket.value.on('game-started', (data) => {
            gameStatus.value = 'playing';
        });

        socket.value.on('new-question', (data) => {
            currentQuestion.value = data.question;
        });

        socket.value.on('score-updated', (data) => {
            players.value = data.players;
        });

        socket.value.on('game-ended', (data) => {
            gameStatus.value = 'finished';
            players.value = data.players;
        });

        socket.value.on('host-changed', (data) => {
            isHost.value = socket.value.id === data.newHost;
        });
    };

    const createGame = (playerName) => {
        socket.value.emit('create-game', { playerName });
    };

    const joinGame = (code, playerName) => {
        gameCode.value = code;
        socket.value.emit('join-game', { gameCode: code, playerName });
    };

    const startGame = () => {
        socket.value.emit('start-game', { gameCode: gameCode.value });
    };

    const sendQuestion = (question) => {
        socket.value.emit('send-question', {
            gameCode: gameCode.value,
            question
        });
    };

    const submitAnswer = (answer, playerName) => {
        socket.value.emit('submit-answer', {
            gameCode: gameCode.value,
            answer,
            playerName
        });
    };

    const validateAnswer = (playerId, isCorrect, points = 1) => {
        socket.value.emit('validate-answer', {
            gameCode: gameCode.value,
            playerId,
            isCorrect,
            points
        });
    };

    const endGame = () => {
        socket.value.emit('end-game', { gameCode: gameCode.value });
    };

    const onAnswerReceived = (callback) => {
        socket.value.on('answer-received', callback);
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
        socket,
        connected,
        gameCode,
        players,
        currentQuestion,
        gameStatus,
        isHost,
        error,
        connect,
        createGame,
        joinGame,
        startGame,
        sendQuestion,
        submitAnswer,
        validateAnswer,
        endGame,
        onAnswerReceived,
        disconnect
    };
}