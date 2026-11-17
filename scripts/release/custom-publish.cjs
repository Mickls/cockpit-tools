const fs = require('node:fs');
const { execFileSync, spawnSync } = require('node:child_process');
const identity = JSON.parse(fs.readFileSync('custom-release.json', 'utf8'));
const repo = process.env.GITHUB_REPOSITORY;
if (repo !== 'Mickls/cockpit-tools') throw new Error('Refusing publication outside the custom fork');
const tag = `v${identity.version}`;
const existing = spawnSync('gh', ['release', 'view', tag, '--json', 'isDraft,targetCommitish'], { encoding: 'utf8' });
if (existing.status === 0) {
  const release = JSON.parse(existing.stdout);
  if (!release.isDraft) throw new Error('Refusing to modify an already public release');
  if (release.targetCommitish !== identity.commit) throw new Error('Release source mismatch');
} else {
  if (!existing.stderr.includes('release not found')) throw new Error('Release lookup failed; refusing mutation');
  execFileSync('gh', ['release', 'create', tag, '--target', identity.commit, '--draft',
    '--title', `Cockpit Tools 自用版 ${identity.version}`, '--notes-file', 'release-notes.md']);
}
const assets = fs.readdirSync('release-assets').map(f => `release-assets/${f}`);
execFileSync(process.execPath, ['scripts/release/upload_release_assets.cjs', tag, ...assets], { stdio: 'inherit' });
