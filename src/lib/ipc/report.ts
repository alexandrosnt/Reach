import { invoke } from '@tauri-apps/api/core';

/** Something private and what it becomes in the shared log. */
export interface Private {
	text: string;
	label: 'host' | 'user' | 'session';
}

export interface Report {
	path: string;
	log: string;
	version: string;
	os: string;
	desktop: string | null;
	problems: string[];
}

/** Make the cleaned copy of reach.log. */
export async function reportPrepare(privateTerms: Private[]): Promise<Report> {
	return invoke<Report>('report_prepare', { private: privateTerms });
}

/** Show the cleaned copy, selected, in the file manager. */
export async function reportReveal(): Promise<void> {
	return invoke('report_reveal');
}
