export const CURRENT_VERSION = '0.1.19';
export const CURRENT_VERSION_TAG = `v${CURRENT_VERSION}`;
export const LATEST_VERSION_LABEL = `Published CLI ${CURRENT_VERSION_TAG}`;
export const GIT_BRANCH = import.meta.env.VITE_GIT_BRANCH || 'source checkout';
export const GITHUB_OWNER = 'morshedulmunna';
export const GITHUB_REPO = 'xecute';
export const GITHUB_URL = `https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}`;
export const GITHUB_API_URL = `https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}`;
