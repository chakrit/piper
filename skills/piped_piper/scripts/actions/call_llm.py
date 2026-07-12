import subprocess

class CallLlm:
    """
    Unit of work for executing a prompt via the 'claude' CLI.
    """
    def __init__(self, prompt: str):
        self.prompt = prompt

    def run(self) -> str:
        try:
            # Using the 'claude' CLI to process the prompt.
            # We assume 'claude' is in the PATH or provided via environment.
            result = subprocess.run(
                ["claude", "-p", self.prompt],
                capture_output=True,
                text=True,
                check=True
            )
            return result.stdout.strip()
        except subprocess.CalledProcessError as e:
            # Propagate the error with context from stderr.
            raise RuntimeError(f"Claude CLI failure: {e.stderr}") from e
        except FileNotFoundError:
            # Handle case where 'claude' binary is not found.
            raise RuntimeError("The 'claude' CLI binary was not found in the system PATH.")
