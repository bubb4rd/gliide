-- Version 4: the salary range you set on a posting, as the serde JSON of `Option<resume_core::salary::Salary>`.
-- SQL NULL: you haven't set one, so the posting's own shows; JSON `null`: you removed it. A re-read of the same text
-- refreshes `json` and leaves this alone.
ALTER TABLE postings ADD COLUMN salary TEXT;
