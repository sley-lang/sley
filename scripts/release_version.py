"""Current candidate identity from the workspace, never a historical release."""
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
VERSION = re.compile(r'(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?')


def workspace_version(root: Path = ROOT) -> str:
    """Read a release-safe Cargo workspace version; missing/invalid is an error."""
    version = tomllib.loads((root/'Cargo.toml').read_text())['workspace']['package']['version']
    if not isinstance(version,str) or VERSION.fullmatch(version) is None:
        raise ValueError('invalid workspace candidate version')
    return version


def artifact_stem(root: Path = ROOT) -> str:
    return f'sley-{workspace_version(root)}-linux-x86_64'


def artifact_name(root: Path = ROOT) -> str:
    return artifact_stem(root)+'.tar.gz'
