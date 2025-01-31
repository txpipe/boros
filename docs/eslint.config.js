const path = require('node:path');
const globals = require('globals');
const stylistic = require('@stylistic/eslint-plugin');
const parserTs = require('@typescript-eslint/parser');
const importPlugin = require('eslint-plugin-import');
const reactHookPlugin = require('eslint-plugin-react-hooks');
const eslintTs = require('@typescript-eslint/eslint-plugin');
const { includeIgnoreFile } = require('@eslint/compat');

const gitignorePath = path.resolve(__dirname, '.gitignore');

const indentSize = 2;

module.exports = [
  includeIgnoreFile(gitignorePath),
  {
    files: ['**/*.{ts,tsx,js,jsx,cjs}'],
    ignores: ['build/**/*', 'app/entry.server.tsx', 'app/entry.client.tsx', 'app/spec/**/*'],
    languageOptions: {
      ecmaVersion: 2020,
      parser: parserTs,
      globals: globals.browser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    plugins: {
      '@stylistic': stylistic,
      import: importPlugin,
      'react-hooks': reactHookPlugin,
      '@typescript-eslint': eslintTs,
    },
    settings: {
      react: {
        version: 'detect',
      },
      formComponents: ['Form'],
      linkComponents: [
        { name: 'Link', linkAttribute: 'to' },
        { name: 'NavLink', linkAttribute: 'to' },
      ],
    },
    rules: {
      ...stylistic.configs['recommended-flat'].rules,
      '@stylistic/semi': ['error', 'always'],
      '@stylistic/jsx-one-expression-per-line': 'off',
      '@stylistic/quotes': ['error', 'single', { avoidEscape: true }],
      '@stylistic/quote-props': ['error', 'as-needed'],
      '@stylistic/indent': ['error', indentSize],
      '@stylistic/jsx-indent': ['error', indentSize],
      '@stylistic/jsx-curly-brace-presence': ['error', 'never'],
      '@stylistic/object-curly-spacing': ['error', 'always'],
      '@stylistic/object-curly-newline': ['error', { multiline: true, consistent: true }],
      '@stylistic/array-bracket-newline': ['error', 'consistent'],
      '@stylistic/max-len': [
        'error',
        {
          code: 120,
          ignoreUrls: true,
          ignoreTemplateLiterals: true,
          ignoreRegExpLiterals: true,
          ignoreComments: true,
          ignoreStrings: true,
        },
      ],
      '@stylistic/member-delimiter-style': ['error', {
        multiline: {
          delimiter: 'semi',
          requireLast: true,
        },
        singleline: {
          delimiter: 'semi',
          requireLast: true,
        },
        multilineDetection: 'brackets',
      }],
      '@stylistic/brace-style': ['error', '1tbs'],
      '@stylistic/arrow-parens': ['error', 'as-needed'],
      '@stylistic/jsx-self-closing-comp': ['error'],
      '@typescript-eslint/no-shadow': 'error',
      '@typescript-eslint/no-unused-vars': ['error', {
        argsIgnorePattern: '^_',
        caughtErrorsIgnorePattern: '^_',
        destructuredArrayIgnorePattern: '^_',
        varsIgnorePattern: '^_',
      }],
      'no-console': 'error',
      'object-shorthand': ['error', 'always'],

      'import/order': [
        'error',
        {
          groups: [
            'builtin',
            'external',
            'internal',
            'parent',
            'sibling',
            'index',
          ],
          pathGroups: [{
            pattern: '~/**',
            group: 'internal',
          }],
        },
      ],
      ...reactHookPlugin.configs.recommended.rules,
    },
  },
];
