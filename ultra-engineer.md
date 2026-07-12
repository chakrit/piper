# 1. Clarify

Given a requirement, analyze, think long and hard, and rewrite the user request into the
most clear instructions possible. Compare it against the specifications that exists in the
repository. If there are any potential questions that cannot be automatically resolve by
the project's standing convention or philosophy, terminate the pipeline with the question
list so the user can give answers.

# 2. Distill

Given the clarified requirement, distill them into work streams, each one a coherent
slices of work that can be done in isolation from each other. FORK the pipeline execution
one work stream per slice of work.

# 3. Architect and Abstract.

Given the slice of work to be done. Analyze, based on specifications and current code
architecture, ask:

- Would new architecture or adjustment to existing one yield a better readable, more
  maintainable, and reliable (bug free) codebase?
- Would a new abstraction or adjustment to existing abstractions help with reducing the
  amount of errors and/or make future implementation easier/more streamlined and
  straight forward?
- Are there ways to simplify the current codebase such that implementing this new request
  becomes easier/more trivial?

If any question result in a YES, then details the adjustments needed and go to the
REFACTOR stage. Otherwise go to TEST-PLAN

# 4. Refactor

Based on findings from the previous steps, refactor the codebase such that implementing
the requested requirement becomes simpler and easier and more straight-forward. Go to
TEST-PLAN when done.

# 5. Test Plan

Think and analyze the given requirements, how one could test that the feature actually
works, and work well. Write down a plan. Go to TDD-RED when done.

# 6. TDD-Red

Write a small slice of tests, in TDD style, failing tests that would actually exercises
the use case stated by the requirement. Align this test according to the TEST-PLAN output.

The test must fails in a way that confirms missing behavior. Running the test suite must
actually result in test-fails at this stage. Go to TDD-GREEN when done.

# 7. TDD-Green

Write as little code as possible to make the failing test pass. Then move to TDD-REFACTOR.

# 8. TDD-Refactor

Look at recent implementations, find opportunities to refactor to improve levels of
abstractions and reduce code duplication. Apply them. Then move to TDD-LOOP

# 9. TDD-Loop

Based on all previous TDD steps, analyze and determine whether the requirement has been
fully fulfilled, answer only YES or NO. If YES, go to AUDIT. If NO, go to TDD-RED

# 10. Audit

Read all recent implementations and verify that both the code written, test written,
and related specs files are all aligned. If not, flag the misalignment and go back to
ARCHITECT step. Otherwise TERMINATE

