-- Version 18: a practiced answer said out loud (spoken practice): how long you spoke (`spoken_ms`, from the
-- microphone opening to its closing, pauses included) and how many words were heard (`spoken_words`). NULL for an
-- answer typed.
ALTER TABLE practice ADD COLUMN spoken_ms INTEGER;
ALTER TABLE practice ADD COLUMN spoken_words INTEGER;
