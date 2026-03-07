# Pact

_A binding agreement._

Pact is an TUI for working with LLMs, allowing you to work with
them in an agentic fashion.

A pact where an individual sacrifices moral integrity, spiritual
values, or their soul to a demonic figure in exchange for worldly
gains like knowledge, power, or wealth.

## Targets

_Purposely restricting these to focus on 

- llama.cpp + OpenAI compatible inference APIs.
- Models
  - Kimi K2.5 (cloud)
  - GLM 4.7 Flash
  - Qwen 3.5 35b a3b
  - Qwen 3.5 9b
  - Qwen 3.5 2b - works but is not smart enough to even ask coding questions
  - Qwen 3 Coder 30b a3b
  - Qwen 3 4b Instruct 2507
  - GPT OSS 20b
  - Llama 3.2 1b

- Optimised Targets
  - Qwen 3.5 27b (slower but smart enough)
  - Gemini 3 Flash (best price/performance for Cloud)
  - Kimi K2.5 (decent price/performance for Cloud)
  - Qwen 3.5 9b (MAY be good enough for coding questions, need to
    test)

If coding directly isn't fast enough (yet) then how about coding
assistance? Will need to do my own local testing.

## Local Model Testing

_Experience I've had with open models on my 32GB DDR5 laptop with
AMD Ryzen 9 270 w/ Radeon 780M Graphics._

- gpt-oss-20b - fast output (24 tokens per second), very verbose
  thinking and not very good at tool calling.
- qwen3-4b instruct 2507 - sends everything as thinking despite
  being an instruct model.. suspect this is a non-model issue I
  haven't figured out.
- qwen3.5 30b a3b - good but slow
- qwen3.5 2b - 32 tokens per second, surprisingly coherent and
  reliable at tool calling. It is not good at coding, reasoning
  is also very basic.
- glm 4.7 flash - 4-5 tokens per second, better than smaller
  models but does not deliver.

### References on models

- [brokk.ai power ranking](https://blog.brokk.ai/the-26-02-coding-power-ranking/)
  - suggests 27b is worth it over 35b a3b in ability even at the
    cost of speed
  - also suggests local models are capable but not fast enough to
    be usable.
  - Gemini 3 Flash recommended as the best option. Kimi K2.5 also
    does very well.

## Anti-features

- sub-agents - haven't blown context yet, keep things simple.
