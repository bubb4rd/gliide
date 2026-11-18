-- Version 17: `applications.sent_template`: the template the resume as sent was exported in, so it renders again as
-- the employer has it after its master switches template. Resumes exported before it were all `classic-serif`, the
-- only template then; NULL where `sent` is.
ALTER TABLE applications ADD COLUMN sent_template TEXT;
UPDATE applications SET sent_template = 'classic-serif' WHERE sent IS NOT NULL;
