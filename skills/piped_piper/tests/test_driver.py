import unittest
from unittest.mock import MagicMock, patch, ANY
from skills.piped_piper.scripts.driver import PipedPiperEngine
from skills.piped_piper.scripts.actions.call_llm import CallLlm
import subprocess

class TestPipedPiperEngine(unittest.TestCase):
    def setUp(self):
        # Create a mock pipeline file
        self.pipeline_path = "tests/mock_pipeline.json"
        self.prompt_dir = "tests/mock_prompts"
        
        import os, json
        os.makedirs("tests/mock_prompts", exist_ok=True)
        with open(self.pipeline_path, 'w') as f:
            json.dump([
                {"name": "Aspect 1", "description": "Desc 1"},
                {"name": "Aspect 2", "description": "Desc 2"}
            ], f)
        
        with open("tests/mock_prompts/aspect-refiner.md", 'w') as f:
            f.write("Refine {{aspect_name}}: {{aspect_description}}\nDraft: {{current_draft}}")
            
        with open("tests/mock_prompts/final-alignment.md", 'w') as f:
            f.write("Goal: {{original_goal}}\nDraft: {{final_draft}}\nAspects: {{aspect_list}}")

    def test_pipeline_loop_updates_draft(self):
        engine = PipedPiperEngine(self.pipeline_path, self.prompt_dir)
        
        # Mock call_llm to return a sequence of refined drafts
        responses = [
            '{"refined_draft": "Draft 1", "violations_found": 0}',
            '{"refined_draft": "Draft 2", "violations_found": 0}',
            'Final Result'
        ]
        engine.call_llm = MagicMock(side_effect=responses)
        
        initial_draft = "Initial Draft"
        goal = "The Goal"
        result = engine.run(initial_draft, goal)
        
        # Verify the draft evolved
        self.assertEqual(engine.working_draft, "Draft 2")
        self.assertEqual(result, "Final Result")
        
        # Verify call_llm was called for each aspect + synthesis
        self.assertEqual(engine.call_llm.call_count, 3)

class TestLlmActions(unittest.TestCase):
    @patch("subprocess.run")
    def test_call_llm_success(self, mock_run):
        # Setup mock response
        mock_run.return_value = MagicMock(
            stdout="LLM Response",
            returncode=0
        )
        
        action = CallLlm("Hello LLM")
        result = action.run()
        
        self.assertEqual(result, "LLM Response")
        mock_run.assert_called_once_with(
            ["claude", "-p", "Hello LLM"],
            capture_output=True,
            text=True,
            check=True
        )

    @patch("subprocess.run")
    def test_call_llm_failure(self, mock_run):
        # Setup mock failure
        mock_run.side_effect = subprocess.CalledProcessError(
            returncode=1,
            cmd=["claude", "-p", "Fail"],
            stderr="API Error"
        )
        
        action = CallLlm("Fail")
        with self.assertRaisesRegex(RuntimeError, "Claude CLI failure: API Error"):
            action.run()

if __name__ == "__main__":
    unittest.main()
