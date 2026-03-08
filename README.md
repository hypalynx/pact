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
  - GPT OSS 20b
  - Llama 3.2 1b

- Optimised Targets
  - Qwen 3.5 27b (slower but smart enough)
  - Qwen 3.5 9b (faster but not as smart)
  - GLM 4.7 Flash
  - Gemini 3 Flash (best price/performance for Cloud)
  - Kimi K2.5 (decent price/performance for Cloud)

If coding directly isn't fast enough (yet) then how about coding
assistance? Will need to do my own local testing.

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
