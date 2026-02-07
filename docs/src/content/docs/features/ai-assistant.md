---
title: AI Assistant
description: Get help from AI right inside Reach.
---

Reach has an optional AI assistant that can help with server administration tasks. It's not required to use the app, but it can save you time when you're troubleshooting or need a quick answer.

## Setup

The AI runs through [OpenRouter](https://openrouter.ai/), so you'll need an API key from them.

1. Go to **Settings > AI**
2. Enable AI features
3. Paste your OpenRouter API key
4. Pick a model (defaults to Claude 3.5 Sonnet, but you can choose any model available on OpenRouter)

Your API key is stored encrypted in the vault, not in plaintext.

## Opening the AI panel

Click the star icon in the title bar, or press **Ctrl+Shift+A**.

## Context awareness

The AI knows what you're doing. It reads the last 50 lines of your terminal output and knows which server you're connected to (host, username, OS). So you can ask something like "why is the disk full?" and it already has enough context to give you a useful answer.

## Running commands

When the AI suggests commands, they show up as code blocks with a **Run** button. Click it and the command gets executed directly in your active terminal. No copy-pasting needed.

## Auto-execution

The AI can run commands, read the output, and then suggest the next step automatically. This goes up to 6 rounds without you needing to intervene. It's useful for debugging scenarios where you need to run a command, look at the result, run another one, and so on.

## Playbook generation

From the playbook editor, you can describe what you want in plain language and the AI generates the YAML for you. Good for quickly scaffolding automation tasks without writing the playbook from scratch.
