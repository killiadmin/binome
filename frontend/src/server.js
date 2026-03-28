import express from 'express';
import { createServer } from 'http';
import { Server } from 'socket.io';

const app = express();
const httpServer = createServer(app);
const io = new Server(httpServer, {
    cors: {
        origin: '*',
        methods: ["GET", "POST"]
    }
});

const games = new Map();

io.on('connection', (socket) => {
    console.log('Nouveau joueur connecté:', socket.id);

    // ── Créer une partie ────────────────────────────────────────────────────────
    socket.on('create-game', (data) => {
        const gameCode = generateGameCode();
        const game = {
            code: gameCode,
            host: socket.id,
            players: [{
                id: socket.id,
                name: data.playerName,
                score: 0,
                character: null,
                found: false
            }],
            status: 'waiting',
            currentTurn: null,
            turnOrder: [],
            pendingQuestion: null,
        };

        games.set(gameCode, game);
        socket.join(gameCode);
        socket.emit('game-created', { gameCode, game: sanitize(game, socket.id) });
        console.log(`Partie créée: ${gameCode}`);
    });

    // ── Rejoindre une partie ────────────────────────────────────────────────────
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

        game.players.push({
            id: socket.id,
            name: data.playerName,
            score: 0,
            character: null,
            found: false
        });
        socket.join(data.gameCode);

        broadcastState(game);
        console.log(`${data.playerName} a rejoint la partie ${data.gameCode}`);
    });

    socket.on('assign-character', (data) => {
        const game = games.get(data.gameCode);
        if (!game || game.host !== socket.id) return;

        const target = game.players.find(p => p.id === data.targetId);
        if (!target) return;

        target.character = data.character;

        io.to(data.targetId).emit('your-character', { character: data.character });

        broadcastState(game);
    });

    socket.on('start-game', (data) => {
        const game = games.get(data.gameCode);
        if (!game || game.host !== socket.id) return;

        const missing = game.players.filter(p => !p.character);
        if (missing.length > 0) {
            socket.emit('error', { message: 'Tous les joueurs doivent avoir un personnage !' });
            return;
        }

        game.status = 'playing';
        game.turnOrder = game.players.map(p => p.id);
        game.currentTurn = game.turnOrder[0];

        broadcastState(game);
        console.log(`Partie ${data.gameCode} démarrée`);
    });

    // ── Poser une question ──────────────────────────────────────────────────────
    socket.on('send-question', (data) => {
        const game = games.get(data.gameCode);
        if (!game || game.status !== 'playing') return;

        const fromPlayer = game.players.find(p => p.id === socket.id);
        const targetPlayer = game.players.find(p => p.id === game.currentTurn);

        game.pendingQuestion = data.question;

        // Broadcast à tout le monde
        io.to(data.gameCode).emit('new-question', {
            fromName: fromPlayer?.name,
            question: data.question,
            targetName: targetPlayer?.name,
            targetId: game.currentTurn,
        });
    });

    // ── Répondre à une question (joueur actif uniquement) ───────────────────────
    socket.on('send-answer', (data) => {
        const game = games.get(data.gameCode);
        if (!game || game.currentTurn !== socket.id) return;

        const player = game.players.find(p => p.id === socket.id);
        game.pendingQuestion = null;

        io.to(data.gameCode).emit('question-answered', {
            fromName: player?.name,
            answer: data.answer,       // 'oui' | 'non' | 'peutetre'
            question: data.question,
        });
    });

    // ── Deviner le personnage ───────────────────────────────────────────────────
    socket.on('submit-guess', (data) => {
        const game = games.get(data.gameCode);
        if (!game || game.status !== 'playing') return;

        const fromPlayer = game.players.find(p => p.id === socket.id);
        const targetPlayer = game.players.find(p => p.id === game.currentTurn);

        if (!targetPlayer) return;

        const correct = data.guess.toLowerCase().trim() === targetPlayer.character.toLowerCase().trim();

        if (correct) {
            fromPlayer.score += 1;
            targetPlayer.found = true;
        }

        io.to(data.gameCode).emit('guess-result', {
            fromName: fromPlayer?.name,
            guess: data.guess,
            targetName: targetPlayer?.name,
            correct,
            revealedCharacter: correct ? targetPlayer.character : null,
        });

        if (correct) nextTurn(game);
    });

    // ── Passer au joueur suivant (hôte uniquement) ──────────────────────────────
    socket.on('next-turn', (data) => {
        const game = games.get(data.gameCode);
        if (!game || game.host !== socket.id) return;
        nextTurn(game);
    });

    // ── Rejouer (hôte uniquement) ───────────────────────────────────────────────
    socket.on('reset-game', (data) => {
        const game = games.get(data.gameCode);
        if (!game || game.host !== socket.id) return;

        game.players.forEach(p => {
            p.character = null;
            p.score = 0;
            p.found = false;
        });
        game.status = 'waiting';
        game.currentTurn = null;
        game.turnOrder = [];
        game.pendingQuestion = null;

        broadcastState(game);
    });

    // ── Déconnexion ─────────────────────────────────────────────────────────────
    socket.on('disconnect', () => {
        console.log('Joueur déconnecté:', socket.id);

        games.forEach((game, code) => {
            const idx = game.players.findIndex(p => p.id === socket.id);
            if (idx === -1) return;

            game.players.splice(idx, 1);

            if (game.players.length === 0) {
                games.delete(code);
                return;
            }

            if (game.host === socket.id) {
                game.host = game.players[0].id;
                io.to(code).emit('host-changed', { newHost: game.host });
            }

            broadcastState(game);
        });
    });
});

function nextTurn(game) {
    const idx = game.turnOrder.indexOf(game.currentTurn);
    const next = idx + 1;

    if (next >= game.turnOrder.length) {
        game.status = 'finished';
        game.currentTurn = null;
    } else {
        game.currentTurn = game.turnOrder[next];
        game.players.find(p => p.id === game.currentTurn).found = false;
    }

    broadcastState(game);
}

function broadcastState(game) {
    game.players.forEach(player => {
        const socket = io.sockets.sockets.get(player.id);
        if (!socket) return;

        socket.emit('game-state', sanitize(game, player.id));
    });
}

function sanitize(game, myId) {
    return {
        code: game.code,
        status: game.status,
        host: game.host,
        currentTurn: game.currentTurn,
        isHost: game.host === myId,
        myId,
        players: game.players.map(p => ({
            id: p.id,
            name: p.name,
            score: p.score,
            found: p.found,
            hasCharacter: !!p.character,
            character: p.id === myId ? p.character : null,
        })),
    };
}

function generateGameCode() {
    return Math.random().toString(36).substring(2, 8).toUpperCase();
}

const PORT = process.env.PORT || 3000;
httpServer.listen(PORT, '0.0.0.0', () => {
    console.log(`Serveur WebSocket démarré sur le port ${PORT}`);
});