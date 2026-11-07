// Conventional Commits 1.0.0, enforced by the commit-msg hook and CI.
// Types drive versioning: feat → minor, fix/perf → patch, `!` or BREAKING CHANGE → major.
export default {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'type-enum': [
      2,
      'always',
      ['feat', 'fix', 'perf', 'refactor', 'test', 'docs', 'build', 'ci', 'chore', 'style', 'revert'],
    ],
    // Optional, but when present it must name a part of Feedrico.
    'scope-enum': [
      2,
      'always',
      ['core', 'shell', 'ui', 'tokens', 'library', 'feeds', 'layouts', 'spec', 'ci', 'deps', 'release', 'site'],
    ],
    'header-max-length': [2, 'always', 80],
    'body-max-line-length': [2, 'always', 100],
  },
};
