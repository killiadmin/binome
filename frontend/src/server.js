// server.js
import express from 'express';
import { createServer } from 'http';
import { Server } from 'socket.io';

const app = express();
const httpServer = createServer(app);
const io = new Server(httpServer, {
    cors: {
        origin: "http://localhost:5173", // Votre URL Vue.js dev
        methods: ["GET", "POST"]
    }
});

const games = new Map();

io.on('connection', (socket) => {
    console.log('Nouveau joueur connecté:', socket.id);

    socket.on('create-game', (data) => {
        const gameCode = generateGameCode();
        const game = {
            code: gameCode,
            host: socket.id,
            players: [{ id: socket.id, name: data.playerName, score: 0 }],
            status: 'waiting', // waiting, playing, finished
            currentQuestion: null,
            questionIndex: 0
        };

        games.set(gameCode, game);
        socket.join(gameCode);
        socket.emit('game-created', { gameCode, game });
        console.log(`Partie créée: ${gameCode}`);
    });

    socket.on('join-game', (data) => {
        const game = games.get(data.gameCode);

        if (!game) {
            socket.emit('error', { message: 'Partie introuvable' });
            return;
        }

        if (game.status !== 'waiting') {
            socket.emit('error', { message: 'La partie a déjà commencé' });
            return;
        }

        const player = { id: socket.id, name: data.playerName, score: 0 };
        game.players.push(player);
        socket.join(data.gameCode);

        // Notifier tous les joueurs
        io.to(data.gameCode).emit('player-joined', { player, game });
        console.log(`${data.playerName} a rejoint la partie ${data.gameCode}`);
    });

    socket.on('start-game', (data) => {
        const game = games.get(data.gameCode);

        if (!game || game.host !== socket.id) {
            socket.emit('error', { message: 'Action non autorisée' });
            return;
        }

        game.status = 'playing';
        io.to(data.gameCode).emit('game-started', { game });
        console.log(`Partie ${data.gameCode} démarrée`);
    });

    socket.on('send-question', (data) => {
        const game = games.get(data.gameCode);

        if (!game || game.host !== socket.id) {
            socket.emit('error', { message: 'Action non autorisée' });
            return;
        }

        game.currentQuestion = data.question;
        game.questionIndex++;

        io.to(data.gameCode).emit('new-question', {
            question: data.question,
            questionIndex: game.questionIndex
        });
    });

    socket.on('submit-answer', (data) => {
        const game = games.get(data.gameCode);

        if (!game) return;

        io.to(game.host).emit('answer-received', {
            playerId: socket.id,
            playerName: data.playerName,
            answer: data.answer,
            timestamp: Date.now()
        });
    });

    socket.on('validate-answer', (data) => {
        const game = games.get(data.gameCode);

        if (!game || game.host !== socket.id) return;

        const player = game.players.find(p => p.id === data.playerId);
        if (player && data.isCorrect) {
            player.score += data.points || 1;
        }

        io.to(data.gameCode).emit('score-updated', {
            players: game.players,
            playerId: data.playerId,
            isCorrect: data.isCorrect
        });
    });

    socket.on('end-game', (data) => {
        const game = games.get(data.gameCode);

        if (!game || game.host !== socket.id) return;

        game.status = 'finished';
        io.to(data.gameCode).emit('game-ended', {
            players: game.players.sort((a, b) => b.score - a.score)
        });
    });

    socket.on('disconnect', () => {
        console.log('Joueur déconnecté:', socket.id);

        games.forEach((game, code) => {
            const playerIndex = game.players.findIndex(p => p.id === socket.id);

            if (playerIndex !== -1) {
                game.players.splice(playerIndex, 1);

                if (game.host === socket.id && game.players.length > 0) {
                    game.host = game.players[0].id;
                    io.to(code).emit('host-changed', { newHost: game.host });
                }

                if (game.players.length === 0) {
                    games.delete(code);
                } else {
                    io.to(code).emit('player-left', { playerId: socket.id, game });
                }
            }
        });
    });
});

function generateGameCode() {
    return Math.random().toString(36).substring(2, 8).toUpperCase();
}

const PORT = process.env.PORT || 3000;
httpServer.listen(PORT, () => {
    console.log(`Serveur WebSocket démarré sur le port ${PORT}`);
});