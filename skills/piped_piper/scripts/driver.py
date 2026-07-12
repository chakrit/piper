import json
import sys
import os
from typing import List, Dict

class PipedPiperEngine:
    def __init__(self, pipeline_path: str, prompt_dir: str):
        with open(pipeline_path, 'r') as f:
            self.pipeline = json.load(f)
        self.prompt_dir = prompt_dir
        self.working_draft = ""

    def _load_prompt(self, filename: str) -> str:
        with open(os.path.join(self.prompt_dir, filename), 'r') as f:
            return f.read()

    def call_llm(self, prompt: str) -> str:
        """
        Hook for LLM integration. 
        In a real scenario, this would call the OpenAI/Anthropic API or a local CLI.
        """
        # This is a placeholder. The user should replace this with their actual LLM client.
        print("\n[Driver] Calling LLM for Aspect processing...")
        # Example: return subprocess.check_output(["claude", "-p", prompt]).decode('utf-8')
        raise NotImplementedError("Please implement call_llm with your preferred LLM client.")

    def run(self, initial_draft: str, original_goal: str = ""):
        self.working_draft = initial_draft
        
        # 1. AOP Pipeline Loop
        refiner_template = self._load_prompt("aspect-refiner.md")
        
        for aspect in self.pipeline:
            print(f"\n>>> Processing Aspect: {aspect['name']}")
            
            # Prepare the prompt for the Worker Lens
            prompt = refiner_template.replace("{{aspect_name}}", aspect['name']) \
                                     .replace("{{aspect_description}}", aspect['description']) \
                                     .replace("{{current_draft}}", self.working_draft)
            
            try:
                response_text = self.call_llm(prompt)
                # Extract JSON from response (handling potential markdown blocks)
                if "```json" in response_text:
                    json_str = response_text.split("```json")[1].split("```")[0].strip()
                else:
                    json_str = response_text.strip()
                
                result = json.loads(json_str)
                self.working_draft = result['refined_draft']
                print(f"[Driver] Aspect {aspect['name']} completed. Violations found: {result['violations_found']}")
            except Exception as e:
                print(f"[Driver] Error processing aspect {aspect['name']}: {e}")
                sys.exit(1)

        # 2. Final Synthesis Pass
        print("\n>>> Performing Final Synthesis...")
        synthesis_template = self._load_prompt("final-alignment.md")
        final_prompt = synthesis_template.replace("{{original_goal}}", original_goal) \
                                          .replace("{{final_draft}}", self.working_draft) \
                                          .replace("{{aspect_list}}", ", ".join([a['name'] for a in self.pipeline]))
        
        final_output = self.call_llm(final_prompt)
        return final_output

def main():
    # Example usage: python driver.py pipelines/piped-eng.json "Goal: Implement X" "Initial draft content..."
    if len(sys.argv) < 4:
        print("Usage: python driver.py <pipeline_json> <goal> <initial_draft>")
        sys.exit(1)
        
    pipeline_file = sys.argv[1]
    goal = sys.argv[2]
    initial_text = sys.argv[3]
    
    # Use relative path from project root
    engine = PipedPiperEngine(pipeline_file, "skills/piped_piper/prompts")
    final_result = engine.run(initial_text, goal)
    print("\n--- FINAL OUTPUT ---\n")
    print(final_result)

if __name__ == "__main__":
    main()
