"""Generate FreeSheet read-aloud audio with Kokoro-82M on the GPU.

Reads tts/jobs.json (made by scripts/tts/extract.js) and writes one Ogg Opus file per
notes section. Run with the venv in tts/.venv:

    tts\\.venv\\Scripts\\python tts\\generate.py [--voice bf_emma] [--bitrate 24k]
"""
import argparse
import json
import os
import subprocess
import sys
import time

import numpy as np
import torch
from kokoro import KPipeline

SR = 24000
HERE = os.path.dirname(os.path.abspath(__file__))


def encode(audio: np.ndarray, out: str, bitrate: str) -> None:
    os.makedirs(os.path.dirname(out), exist_ok=True)
    tmp = out + ".part"
    cmd = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "f32le", "-ar", str(SR), "-ac", "1",
           "-i", "pipe:0", "-c:a", "libopus", "-b:a", bitrate, "-application", "voip", "-f", "ogg", tmp]
    subprocess.run(cmd, input=audio.astype(np.float32).tobytes(), check=True)
    os.replace(tmp, out)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--voice", default="bf_emma")
    ap.add_argument("--bitrate", default="24k")
    ap.add_argument("--speed", type=float, default=1.0)
    args = ap.parse_args()

    with open(os.path.join(HERE, "jobs.json"), encoding="utf-8") as f:
        jobs = json.load(f)
    if not jobs:
        print("Nothing to do: all audio is up to date.")
        return 0

    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"Kokoro on {device} ({torch.cuda.get_device_name(0) if device == 'cuda' else 'no GPU'}), voice {args.voice}, {len(jobs)} sections")
    pipe = KPipeline(lang_code="b", repo_id="hexgrad/Kokoro-82M", device=device)  # 'b' = British English
    gap = np.zeros(int(SR * 0.35), dtype=np.float32)

    start = time.time()
    seconds = 0.0
    for n, job in enumerate(jobs, 1):
        parts = []
        # Each line of the section text is a block; Kokoro further splits long lines itself.
        for _, _, audio in pipe(job["text"], voice=args.voice, speed=args.speed, split_pattern=r"\n+"):
            if audio is None:
                continue
            parts.append(audio.detach().cpu().numpy() if hasattr(audio, "detach") else np.asarray(audio))
            parts.append(gap)
        if not parts:
            print(f"  skipped (no audio): {job['label']}")
            continue
        clip = np.concatenate(parts)
        encode(clip, job["out"], args.bitrate)
        seconds += len(clip) / SR
        if n % 25 == 0 or n == len(jobs):
            el = time.time() - start
            eta = el / n * (len(jobs) - n)
            print(f"  {n}/{len(jobs)}  {seconds / 3600:.1f} h of audio  elapsed {el / 60:.1f} min  eta {eta / 60:.1f} min", flush=True)
    print("Done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
