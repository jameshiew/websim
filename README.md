# websim

> ⚠️ This project is experimental and shared as-is.
> Expect rough edges, vibe-coded logic, and unreviewed code — use at your own risk!

LLM website simulator, useable in the browser. Requires an [OpenRouter](https://openrouter.ai) API key.

## Usage

```shell
export WEBSIM_API_KEY="..."  # OpenRouter API key
just run  # starts server on localhost:3000
```

Navigate in a browser to an URL you want to generate e.g. http://localhost:3000/simplewiki/articles/2025

Also handles responding to JSON POST requests e.g.

```shell
curl -X POST http://localhost:3000/contact \
    -H "Content-Type: application/json" \
    -d '{"name": "John Doe", "text": "Lorem ipsum dolor sit amet, consectetur adipiscing elit."}'
```

Can be configured via [websim.config.yml](./websim.config.yml)
