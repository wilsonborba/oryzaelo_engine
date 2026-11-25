"""Download the exact academic checkpoints and check them before installation."""
import hashlib
import json
from pathlib import Path
import shutil
from tempfile import NamedTemporaryFile

from huggingface_hub import hf_hub_download

weights = json.loads(Path(__file__).with_name("checkpoints.json").read_text())
for repo, spec in weights.items():
    file = Path(hf_hub_download(repo_id=repo, filename=spec["filename"], revision=spec["revision"]))
    with file.open("rb") as stream:
        if hashlib.file_digest(stream, "sha256").hexdigest() != spec["sha256"]:
            raise ValueError(f"Checksum mismatch for {repo}")
    destination = Path.home() / ".cache/oryzelo-models" / spec["revision"] / spec["filename"]
    destination.parent.mkdir(parents=True, exist_ok=True)
    with NamedTemporaryFile(dir=destination.parent, delete=False) as tmp, file.open("rb") as stream:
        shutil.copyfileobj(stream, tmp)
    Path(tmp.name).replace(destination)
    print(destination)
