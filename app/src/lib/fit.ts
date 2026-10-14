// A job's fit as `ScoreBar` draws it, wherever a fit score shows (Applications, Feed, the sidebar's Recent, the
// Overview): an application's scores when there is one, else the posting's fit score.

import type { ApplicationRow, JobRow } from "$lib/api";

/** What `ScoreBar` takes: projected (a posting not sent yet: its fit score), before your edits and on file. */
export interface Fit {
  now: number | null;
  projected: number;
  onFile: number | null;
}

/** An application's scores: projected (else on file, for a row imported without it), before and on file. */
export function applicationFit(a: ApplicationRow): Fit | null {
  const projected = a.projected ?? a.onFile;
  return projected === null ? null : { now: a.now, projected, onFile: a.onFile };
}

/** A fit score alone (a posting not sent yet, a feed job). */
export function scoreFit(score: number | null): Fit | null {
  return score === null ? null : { now: null, projected: score, onFile: null };
}

/** The posting's fit: its application's scores once it has one, else its own fit score. */
export function jobFit(job: JobRow, applications: ApplicationRow[]): Fit | null {
  const application = job.status === null ? undefined : applications.find((a) => a.job === job.id);
  return (application && applicationFit(application)) ?? scoreFit(job.score);
}
