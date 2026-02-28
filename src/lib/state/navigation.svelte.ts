export type Page = 'terminal' | 'ansible' | 'tofu';

let activePage = $state<Page>('terminal');

export function getActivePage(): Page {
	return activePage;
}

export function setActivePage(page: Page): void {
	activePage = page;
}
