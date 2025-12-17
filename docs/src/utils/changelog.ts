import fs from "fs";
import path from "path";

export interface ChangelogEntry {
	text: string;
	scope?: string;
}

export interface ChangelogSection {
	title: string;
	emoji: string;
	entries: ChangelogEntry[];
}

export interface ChangelogRelease {
	version: string;
	date: string;
	sections: ChangelogSection[];
}

const SECTION_EMOJIS: Record<string, string> = {
	Features: "🚀",
	"Bug Fixes": "🐛",
	Refactor: "🚜",
	Documentation: "📚",
	Performance: "⚡",
	Styling: "🎨",
	Testing: "🧪",
	"Miscellaneous Tasks": "⚙️",
	Security: "🛡️",
	Revert: "◀️",
	Other: "💼",
};

export function parseChangelog(limit = 3): ChangelogRelease[] {
	const changelogPath = path.resolve(process.cwd(), "../CHANGELOG.md");

	if (!fs.existsSync(changelogPath)) {
		console.warn("CHANGELOG.md not found at", changelogPath);
		return [];
	}

	const content = fs.readFileSync(changelogPath, "utf-8");
	const releases: ChangelogRelease[] = [];

	// Split by release headers: ## [version] - date
	const releaseRegex = /^## \[(.+?)\] - (\d{4}-\d{2}-\d{2})/gm;
	const releaseSections = content.split(/(?=^## \[)/m).filter(Boolean);

	for (const section of releaseSections) {
		if (releases.length >= limit) break;

		const headerMatch = section.match(/^## \[(.+?)\] - (\d{4}-\d{2}-\d{2})/);
		if (!headerMatch) continue;

		const [, version, date] = headerMatch;
		const sections: ChangelogSection[] = [];

		// Extract sections (### Title)
		const sectionRegex = /^### (.+?)$([\s\S]*?)(?=^### |$(?![\s\S]))/gm;
		let sectionMatch;

		while ((sectionMatch = sectionRegex.exec(section)) !== null) {
			const rawTitle = sectionMatch[1].trim();
			const sectionContent = sectionMatch[2];

			// Extract emoji and title
			const emojiMatch = rawTitle.match(/^(.+?)\s+(.+)$/);
			let emoji = "";
			let title = rawTitle;

			if (emojiMatch) {
				emoji = emojiMatch[1];
				title = emojiMatch[2];
			}

			// Skip miscellaneous tasks (release commits, etc.)
			if (title === "Miscellaneous Tasks") continue;

			// Parse entries
			const entries: ChangelogEntry[] = [];
			const entryRegex = /^- (?:\*\((.+?)\)\* )?(.+)$/gm;
			let entryMatch;

			while ((entryMatch = entryRegex.exec(sectionContent)) !== null) {
				const scope = entryMatch[1];
				const text = entryMatch[2].trim();

				// Skip release commits
				if (text.toLowerCase().startsWith("release")) continue;

				entries.push({ text, scope });
			}

			if (entries.length > 0) {
				sections.push({ title, emoji, entries });
			}
		}

		if (sections.length > 0) {
			releases.push({ version, date, sections });
		}
	}

	return releases;
}

export function formatDate(dateStr: string): string {
	const date = new Date(dateStr);
	return date.toLocaleDateString("en-US", {
		month: "short",
		day: "numeric",
		year: "numeric",
	});
}
