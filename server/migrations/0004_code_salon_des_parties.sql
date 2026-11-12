-- Chaque partie mémorise le code de son salon.
--
-- Depuis la migration 2, une partie survit à la suppression de son salon, mais
-- l'historique perdait alors le code du salon (lu dans `rooms`). Le code est
-- désormais copié dans la partie à son lancement ; les parties existantes le
-- reçoivent ici depuis leur salon s'il existe encore (NULL sinon : perdu avant).

ALTER TABLE games ADD COLUMN room_code TEXT;
UPDATE games SET room_code = (SELECT code FROM rooms WHERE rooms.id = games.room_id) WHERE room_id IS NOT NULL;
