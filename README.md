# vision-recognizer

OpenAI Vision API MCP stdio server for image recognition and video analysis.

## Getting started

Этот проект разработан и сопровождается через [Claude Code](https://claude.com/claude-code). Чтобы войти в контекст:

1. Клонировать репу.
2. Открыть Claude Code в её корне.
3. Запустить slash-command:

   ```
   /onboarding
   ```

   Skill соберёт информацию о проекте (структура, стек, ADR-ы, как запустить локально) и выведет deliverable-style summary.

## Video analysis

The MCP tool `vision.analyze_video` extracts frames from a short video file
and sends them to OpenAI Vision API (gpt-4o) for analysis.

### Prerequisites

`ffmpeg` must be installed and available in `PATH`:

```bash
brew install ffmpeg
```

### JSON-RPC example

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "vision.analyze_video",
    "arguments": {
      "video_path": "/tmp/squat.mp4",
      "prompt": "Describe the movement pattern, posture, and any form issues.",
      "fps": 2.0
    }
  }
}
```

### Parameters

| Parameter    | Type            | Required | Description                                                   |
|-------------|-----------------|----------|---------------------------------------------------------------|
| `video_path` | `string`        | yes      | Absolute path to the video file (mp4, mov, webm; max 30 sec) |
| `prompt`     | `string`        | yes      | Natural language question / instruction for the model         |
| `fps`        | `number` (f32)  | no       | Frame extraction rate (default: 2.0; capped at 16 frames)     |

### Constraints

- Maximum video duration: **30 seconds**.
- Maximum frames extracted: **16** (fps is automatically reduced if needed).
- Supported formats: `mp4`, `mov`, `webm`.
- Model is always `gpt-4o`.

## License

[MIT](./LICENSE).
