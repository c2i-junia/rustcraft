module.exports = {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'type-enum': [
      2, // Error level
      'always',
      [
        'build', 
        'ci', 
        'chore', 
        'docs', 
        'feat', 
        'fix', 
        'perf', 
        'refactor', 
        'revert', 
        'style', 
        'test',
      ],
    ],
  },
};

