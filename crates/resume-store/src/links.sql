-- Version 13: the link you set on a posting (an `http`/`https` URL). NULL: you haven't set one, so the link it was
-- read from shows (none for pasted text). A re-read of the same text refreshes `json` and leaves this alone.
ALTER TABLE postings ADD COLUMN url TEXT;
