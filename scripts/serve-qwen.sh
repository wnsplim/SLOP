#!/usr/bin/env sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

if [ "${LLAMA_SERVER+x}" ]; then
    server=$LLAMA_SERVER
elif [ -x "$project_root/llm/llama-server" ]; then
    server="$project_root/llm/llama-server"
else
    server="$project_root/llm/llama-b11195/llama-server"
fi

if [ "${SLOP_MODEL_PATH+x}" ]; then
    model=$SLOP_MODEL_PATH
elif [ -f "$project_root/llm/model.gguf" ]; then
    model="$project_root/llm/model.gguf"
else
    model="$project_root/llm/Qwen3-4B-Instruct-2507-Q4_K_M.gguf"
fi

threads=${SLOP_THREADS:-32}
context=${SLOP_CONTEXT:-32768}

if [ ! -x "$server" ]; then
    echo "llama-server not found or not executable: $server" >&2
    echo "Create llm/llama-server or set LLAMA_SERVER." >&2
    exit 1
fi
if [ ! -f "$model" ]; then
    echo "model not found: $model" >&2
    echo "Create llm/model.gguf or set SLOP_MODEL_PATH." >&2
    exit 1
fi

server_dir=$(dirname -- "$server")
export LD_LIBRARY_PATH="$server_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

exec "$server" \
    --model "$model" \
    --alias slop-qwen \
    --threads "$threads" \
    --ctx-size "$context" \
    --host 127.0.0.1 \
    --port 8080 \
    "$@"
