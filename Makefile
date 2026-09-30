# vex: run the readme step by step.
#
#   make help          list steps in run order
#   make 05-check-fib  run one step
#   make pass          steps 05-10 (all must succeed)
#   make fail          steps 11-13 (all must fail with exit 1, as intended)
#   make all           everything in order
#
# Override the launcher to test another checkout:  make VEX=/path/to/bin/vex ...

VEX ?= bin/vex
BASIC := demo/basic.ex
MAIL := demo/mailbox/mailbox.tla

.DEFAULT_GOAL := help
.PHONY: help all pass fail clean \
	01-prereqs 02-setup 03-build 04-doctor \
	05-check-fib 06-check-basic \
	07-engine-z3 07-engine-boogie 07-engine-lean 07-engine-all \
	08-emit-lean 09-check-counter 10-model-mailbox \
	11-model-unfair 12-fail-post 13-fail-race \
	14-report 15-validate

# Steps print as "target  description" from the ## comments, in file order.
help: ## show this list
	@grep -E '^[0-9a-z-]+:.*## ' $(MAKEFILE_LIST) | sed -E 's/:.*## /\t/' | awk -F'\t' '{printf "  %-20s %s\n", $$1, $$2}'
	@printf '  %-20s %s\n' pass 'steps 05-10 (expected to succeed)' fail 'steps 11-13 (expected to fail with exit 1)' all 'everything in order' clean 'remove .vex/run_* artifacts'

# ---- setup ----------------------------------------------------------------

01-prereqs: ## check required tools are installed
	@for t in cargo elixir z3 dotnet java elan python3 curl rg; do \
	  command -v $$t >/dev/null && echo "ok      $$t" || { echo "MISSING $$t"; exit 2; }; done

02-setup: 01-prereqs ## install Boogie, TLC, Lean 4.28.0; build everything (bin/setup.sh)
	bin/setup.sh

03-build: ## build the Rust workspace only
	cargo build --locked --workspace

04-doctor: 03-build ## print tool versions (z3, boogie, lean, tlc, java)
	target/debug/vex doctor

# ---- contracts: Elixir -> Z3 / Boogie / Lean ---------------------------------

05-check-fib: ## ghost defs, accumulator invariants, termination (demo/fib.ex)
	$(VEX) check demo/fib.ex

06-check-basic: ## simple contracts, default engine auto (demo/basic.ex)
	$(VEX) check $(BASIC)

07-engine-z3: ## same file, direct Z3 obligations
	$(VEX) check $(BASIC) --engine z3

07-engine-boogie: ## same file, real Boogie programs
	$(VEX) check $(BASIC) --engine boogie

07-engine-lean: ## same file, Lean theorems with axiom audit
	$(VEX) check $(BASIC) --engine lean

07-engine-all: ## every obligation must pass z3 + boogie + lean
	$(VEX) check $(BASIC) --engine all

08-emit-lean: ## write plan.json/source.json/one .lean file per condition, no proving
	$(VEX) emit $(BASIC) --engine lean

# ---- models: TLA+ / TLC ----------------------------------------------------

09-check-counter: ## contract + TLA+ model importing the generated step() (demo/counter)
	$(VEX) check --config demo/counter/vex.toml

10-model-mailbox: ## bounded mailbox, eventual delivery under weak fairness (passes)
	$(VEX) model $(MAIL)

# ---- expected failures: each must exit 1 (counterexample kept) ----------------

11-model-unfair: ## mailbox without fairness: temporal counterexample (exit 1)
	@$(VEX) model $(MAIL) --cfg demo/mailbox/unfair.cfg; rc=$$?; \
	  test $$rc -eq 1 && echo "expected: exit 1 (liveness counterexample)" || { echo "unexpected exit $$rc"; exit 1; }

12-fail-post: ## wrong postcondition: SMT counterexample (exit 1)
	@$(VEX) check demo/fail/post.ex; rc=$$?; \
	  test $$rc -eq 1 && echo "expected: exit 1 (SMT counterexample)" || { echo "unexpected exit $$rc"; exit 1; }

13-fail-race: ## non-atomic read/write: lost-update trace (exit 1)
	@$(VEX) model demo/fail/race.tla; rc=$$?; \
	  test $$rc -eq 1 && echo "expected: exit 1 (lost-update trace)" || { echo "unexpected exit $$rc"; exit 1; }

# ---- inspect and validate -----------------------------------------------------

14-report: ## summarize .vex/report.json from the last run
	@python3 -c "import json;r=json.load(open('.vex/report.json'));print(json.dumps(r,indent=2)[:4000])"

15-validate: ## full repo validation (bin/check.sh)
	bin/check.sh

# ---- groups ---------------------------------------------------------------------

pass: 05-check-fib 06-check-basic 07-engine-z3 07-engine-boogie 07-engine-lean 07-engine-all 08-emit-lean 09-check-counter 10-model-mailbox

fail: 11-model-unfair 12-fail-post 13-fail-race

all: 01-prereqs 02-setup 04-doctor
	$(MAKE) pass
	$(MAKE) fail
	$(MAKE) 14-report

clean:
	rm -rf .vex/run_*
