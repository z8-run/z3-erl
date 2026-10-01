# Commands appear in run order; `make help` uses the descriptions below.
# make check src=demo/fib.ex engine=boogie
# make check-dafny sample=compile engine=z3
# make test-dafny case=test_dafny.dafny.test_maximum

vex ?= bin/vex
src ?= demo/basic.ex
engine ?= z3
jobs ?= 4
timeout ?= $(if $(filter lean all,$(engine)),30000,10000)
out ?= .vex
sample ?= max
case ?= test_dafny

flags = --engine $(engine) --jobs $(jobs) --timeout $(timeout) --out "$(out)"
dafny := demo/dafny
dafny_max := $(dafny)/max.ex
dafny_list := $(dafny)/list.ex
dafny_env := $(dafny)/env.ex
dafny_expr := $(dafny_env) $(dafny)/expr.ex
dafny_rewrite := $(dafny_expr) $(dafny)/rewrite.ex
dafny_stack := $(dafny_env) $(dafny_list) $(dafny)/stack.ex
dafny_compile := $(dafny_stack) $(dafny)/expr.ex $(dafny)/compile.ex
dafny_parse := $(dafny)/parse.ex
dafny_all := $(wildcard $(dafny)/*.ex)

.DEFAULT_GOAL := help
.PHONY: help prereqs setup build doctor check check-fib check-basic check-paper \
	engine-z3 engine-boogie engine-lean engine-all emit-lean check-dafny test-dafny \
	check-counter model-mailbox model-unfair fail-post fail-race report fmt validate \
	pass fail all clean

help: ## show commands in file order
	@awk 'BEGIN {FS=":.*## "} /^[a-z][a-z0-9-]*:.*## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@printf '\n  Options: src=%s engine=%s jobs=%s timeout=%s out=%s\n' '$(src)' '$(engine)' '$(jobs)' '$(timeout)' '$(out)'
	@printf '  Dafny samples: max list env expr rewrite stack compile parse all\n'
	@printf '  Focus one regression: make test-dafny case=test_dafny.dafny.test_maximum\n'

prereqs: ## check required tools are installed
	@for t in cargo elixir z3 dotnet java elan python3 curl rg; do \
	  command -v $$t >/dev/null && echo "ok      $$t" || { echo "MISSING $$t"; exit 2; }; done

setup: prereqs ## install pinned backends and build everything
	bin/setup.sh

build: ## build the Rust workspace once for direct CLI and test runs
	cargo build --locked --workspace

doctor: ## print installed backend versions
	$(vex) doctor

check: ## verify src with engine, jobs and timeout (milliseconds)
	$(vex) check $(src) $(flags)

check-fib: ## Fibonacci, accumulator invariants and termination
	$(vex) check demo/fib.ex $(flags)

check-basic: ## basic pure contracts
	$(vex) check demo/basic.ex $(flags)

check-paper: ## clauses, tuples, matching and structural recursion
	$(vex) check demo/paper.ex $(flags)

engine-z3: ## check src through direct Z3
	$(MAKE) check engine=z3

engine-boogie: ## check src through Boogie
	$(MAKE) check engine=boogie

engine-lean: ## check src with Lean 4.28.0 and axiom audit
	$(MAKE) check engine=lean

engine-all: ## require Z3, Boogie and Lean for every obligation in src
	$(MAKE) check engine=all

emit-lean: ## emit Lean obligations for src without proving them
	$(vex) emit $(src) --engine lean --out "$(out)"

check-dafny: ## verify one sample and its dependencies; sample=all checks the directory
	$(if $(dafny_$(sample)),,$(error unknown sample '$(sample)'; see make help))
	$(vex) check $(dafny_$(sample)) $(flags)

test-dafny: build ## run proof, runtime and mutation regressions; case selects a test
	PYTHONPATH=tests python3 -m unittest $(case) -v

check-counter: ## check contracts and the TLA+ model using the generated transition
	$(vex) check --config demo/counter/vex.toml $(flags)

model-mailbox: ## check bounded mailboxes with weak fairness
	$(vex) model demo/mailbox/mailbox.tla --out "$(out)"

model-unfair: ## expect a liveness counterexample without fairness (exit 1)
	@$(vex) model demo/mailbox/mailbox.tla --cfg demo/mailbox/unfair.cfg --out "$(out)"; rc=$$?; \
	  test $$rc -eq 1 && echo "expected: exit 1 (liveness counterexample)" || { echo "unexpected exit $$rc"; exit 1; }

fail-post: ## expect a false-postcondition failure (exit 1)
	@$(vex) check demo/fail/post.ex $(flags); rc=$$?; \
	  test $$rc -eq 1 && echo "expected: exit 1 (false postcondition)" || { echo "unexpected exit $$rc"; exit 1; }

fail-race: ## expect a lost-update trace (exit 1)
	@$(vex) model demo/fail/race.tla --out "$(out)"; rc=$$?; \
	  test $$rc -eq 1 && echo "expected: exit 1 (lost-update trace)" || { echo "unexpected exit $$rc"; exit 1; }

report: ## summarize the last report, failures and artifact location
	@python3 -c 'import collections,json,sys; r=json.load(open(sys.argv[1])); print("success:",r["success"]," engine:",r["engine"]); print("contracts:",dict(collections.Counter(c["status"] for c in r["contracts"]))); print("evidence:",dict(collections.Counter(e["status"] for e in r["evidence"]))); print("artifacts:",r["artifacts"]); [print(e["engine"],e["status"],e["detail"].split("\n",1)[0],e["log"]) for e in r["evidence"] if e["status"] != "proved"]; [print("model:",m) for m in r["models"]]' "$(out)/report.json"

fmt: ## format Rust, the Elixir frontend and Dafny translations
	cargo fmt --all
	cd front && ERL_FLAGS='+S 2:2' mix format

validate: ## run all required repository checks (bin/check.sh)
	bin/check.sh

# Recursive recipes keep these groups ordered even under `make -j`.
pass: ## run passing demos once each; basic contracts use all three engines
	$(MAKE) check-fib
	$(MAKE) check-basic engine=all
	$(MAKE) check-paper
	$(MAKE) check-dafny sample=all
	$(MAKE) emit-lean
	$(MAKE) check-counter
	$(MAKE) model-mailbox

fail: ## run all expected failures, checking their exit codes
	$(MAKE) model-unfair
	$(MAKE) fail-post
	$(MAKE) fail-race

all: ## set up tools, then run passing and failing demos in order
	$(MAKE) setup
	$(MAKE) doctor
	$(MAKE) pass
	$(MAKE) fail
	$(MAKE) report

clean: ## remove run artifacts under out
	rm -rf "$(out)"/run_*
