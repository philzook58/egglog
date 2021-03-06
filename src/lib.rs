use egg::*;
use std::fmt::Write;
use std::sync::Arc;
mod gensym;
mod logic;
mod types;
pub use types::*;
use Entry::*;
use EqWrap::*;
use Term::*;
//use types::Directive::*;
mod parser;
pub use parser::*;

fn merge_subst2(s1: &Subst, s2: &Subst) -> Option<Subst> {
    let mut s1 = s1.clone();
    for (v, i) in s2.vec.iter() {
        match s1.insert(*v, *i) {
            // Oh actually we should check
            Some(i1) => {
                if *i != i1 {
                    return None;
                }
            }
            None => (),
        }
    }
    return Some(s1);
}

fn merge_substs(substs1: &Vec<Subst>, substs2: &Vec<Subst>) -> Vec<Subst> {
    // s1s.iter()
    //    .flat_map(|s1| s2s.iter().filter_map(move |s2| merge_subst2(s1, s2)))
    //    .collect()
    let mut substs = vec![]; // this is merge substs above.
    for subst1 in substs1 {
        for subst2 in substs2 {
            if let Some(subst) = merge_subst2(subst1, subst2) {
                substs.push(subst);
            }
        }
    }
    substs
}

#[derive(Debug, PartialEq, Clone)]
struct MultiPattern<S> {
    patterns: Vec<S>,
}
use std::fmt;

impl<S> fmt::Display for MultiPattern<S>
where
    S: fmt::Display,
{
    fn fmt(&self, buf: &mut fmt::Formatter) -> fmt::Result {
        let mut iter = self.patterns.iter();
        if let Some(item) = iter.next() {
            write!(buf, "{}", item)?;
            for item in iter {
                write!(buf, ", {}", item)?;
            }
        }
        Ok(())
    }
}

impl<L: Language, A: Analysis<L>> Searcher<L, A> for EqWrap<Pattern<L>> {
    fn search_eclass(&self, egraph: &EGraph<L, A>, eclass: Id) -> Option<SearchMatches<L>> {
        match self {
            Bare(p) => p.search_eclass(egraph, eclass),
            /* match p.search_eclass(egraph, eclass) {
                None => None,
                Some(matches) => {
                    Some(SearchMatches {
                        ast : None,
                        ..matches
                    })
                }
            } */
            Eq(p1, p2) => {
                let matches = p1.search_eclass(egraph, eclass)?;
                let matches2 = p2.search_eclass(egraph, eclass)?;
                let substs = merge_substs(&matches.substs, &matches2.substs);
                if substs.len() == 0 {
                    None
                } else {
                    Some(SearchMatches {
                        eclass,
                        substs,
                        ast: None,
                    })
                }
            }
        }
    }
    fn vars(&self) -> Vec<egg::Var> {
        match self {
            Bare(p) => p.vars(),
            Eq(l, r) => {
                let mut vars = l.vars();
                vars.extend(r.vars());
                vars
            }
        }
    }
}

impl<L: Language, A: Analysis<L>, P: Searcher<L, A>> Searcher<L, A> for MultiPattern<P> {
    fn search_eclass(&self, egraph: &EGraph<L, A>, eclass: Id) -> Option<SearchMatches<L>> {
        let mut iter = self.patterns.iter();
        let firstpat = iter.next()?;
        let searchmatches = firstpat.search_eclass(egraph, eclass)?;
        let mut matches = searchmatches.substs;
        for pat in iter {
            let mut temp_matches = vec![];
            for pmatch in pat.search(egraph) {
                temp_matches.append(&mut merge_substs(&matches, &pmatch.substs));
            }
            matches = temp_matches;
        }
        Some(SearchMatches {
            eclass,
            substs: matches,
            ast: None,
        })
    }
    fn vars(&self) -> Vec<egg::Var> {
        let mut pats: Vec<_> = self
            .patterns
            .iter()
            .flat_map(|p| <Searcher<L, A>>::vars(p))
            .collect();
        pats.sort();
        pats.dedup();
        pats
    }
}

impl<N, L, A> Applier<L, N> for MultiPattern<A>
where
    L: Language,
    N: Analysis<L>,
    A: Applier<L, N>,
{
    fn apply_one(
        &self,
        egraph: &mut EGraph<L, N>,
        eclass: Id,
        subst: &Subst,
        searcher_ast: Option<&PatternAst<L>>,
        rule_name: Arc<str>,
    ) -> Vec<Id> {
        let mut added = vec![]; // added are union updates, of which there are none
        for applier in &self.patterns {
            added.extend(applier.apply_one(egraph, eclass, subst, searcher_ast, rule_name.clone()));
        }
        added
    }

    fn apply_matches(
        &self,
        egraph: &mut EGraph<L, N>,
        matches: &[SearchMatches<L>],
        rule_name: Arc<str>,
    ) -> Vec<Id> {
        let mut added = vec![];
        for applier in &self.patterns {
            added.extend(applier.apply_matches(egraph, matches, rule_name.clone()));
        }
        added
    }

    fn vars(&self) -> Vec<egg::Var> {
        let mut vars = vec![];
        for applier in &self.patterns {
            vars.extend(applier.vars());
        }
        // Is this necessary? How is var even used?
        vars.sort();
        vars.dedup();
        vars
    }
}

impl<N, L> Applier<L, N> for EqWrap<Pattern<L>>
where
    L: Language,
    N: Analysis<L>,
{
    fn apply_one(
        &self,
        _egraph: &mut EGraph<L, N>,
        _eclass: Id,
        _subst: &Subst,
        searcher_ast: Option<&PatternAst<L>>,
        rule_name: Arc<str>,
    ) -> Vec<Id> {
        // self.0.apply_one(egraph, eclass, subst)
        panic!("EqApply.apply_one was called");
    }

    // Could copy using apply_pat for better efficiency
    fn apply_matches(
        &self,
        egraph: &mut EGraph<L, N>,
        matches: &[SearchMatches<L>],
        rule_name: Arc<str>,
    ) -> Vec<Id> {
        match self {
            Bare(a) =>
            //a.apply_matches(egraph, matches, rule_name)
            {
                // Ignoreapply semantics
                let mut added = vec![];
                for mat in matches {
                    for subst in &mat.substs {
                        let ast = a.ast.as_ref();
                        let mut id_buf = vec![0.into(); ast.len()];
                        let id = apply_pat(&mut id_buf, ast, egraph, subst);
                        added.push(id)
                        // root is just ignored?
                    }
                }
                // TODO: REALLY THINK ABOUT THIS!!!!
                added
            }

            Eq(l, r) => {
                let mut added = vec![]; // added are union updates, of which there are none
                for mat in matches {
                    for subst in &mat.substs {
                        // This should be ok because we know they are patterns. Not very safe.
                        //let id1 = l.apply_one(egraph, 0.into(), subst, None, rule_name)[0];
                        //let id2 = r.apply_one(egraph, 0.into(), subst, None, rule_name)[0];
                        let (to, did_something) =
                            egraph.union_instantiations(&l.ast, &r.ast, subst, rule_name.clone());
                        if did_something {
                            added.push(to)
                        }
                    }
                }
                added
            }
        }
    }

    fn vars(&self) -> Vec<egg::Var> {
        match self {
            Bare(a) => a.vars(),
            Eq(l, r) => {
                let mut vars = l.vars();
                vars.extend(r.vars());
                vars
            }
        }
    }
}

/*
struct EqApply<L> {
    l: Pattern<L>,
    r: Pattern<L>,
}
// Hmm. Should I dfefine an applier for EqWrap<Pattern> instead?
impl<N, L> Applier<L, N> for EqApply<L>
where
    L: Language,
    N: Analysis<L>,
{
    fn apply_one(&self, _egraph: &mut EGraph<L, N>, _eclass: Id, _subst: &Subst, searcher_ast: Option<&PatternAst<L>>,
        rule_name: Arc<str>) -> Vec<Id> {
        // self.0.apply_one(egraph, eclass, subst)
        panic!("EqApply.apply_one was called");
    }

    // Could copy using apply_pat for better efficiency
    fn apply_matches(&self, egraph: &mut EGraph<L, N>, matches: &[SearchMatches<L>], rule_name: Arc<str>) -> Vec<Id> {
        let mut added = vec![]; // added are union updates, of which there are none
        for mat in matches {
            for subst in &mat.substs {
                // This should be ok because we know they are patterns. Not very safe.
                let id1 = self.l.apply_one(egraph, 0.into(), subst, None, rule_name)[0];
                let id2 = self.r.apply_one(egraph, 0.into(), subst, None, rule_name)[0];
                let (to, did_something) = egraph.union(id1, id2);
                if did_something {
                    added.push(to)
                }
            }
        }
        added
    }

    fn vars(&self) -> Vec<egg::Var> {
        let mut vars = self.l.vars();
        vars.extend(self.r.vars());
        vars
    }
}
*/

// Could probably generalize from pattern.
struct IgnoreApply<L>(Pattern<L>);

impl<N, L> Applier<L, N> for IgnoreApply<L>
where
    L: Language,
    N: Analysis<L>,
{
    fn apply_one(
        &self,
        egraph: &mut EGraph<L, N>,
        eclass: Id,
        subst: &Subst,
        searcher_ast: Option<&PatternAst<L>>,
        rule_name: Arc<str>,
    ) -> Vec<Id> {
        self.0
            .apply_one(egraph, eclass, subst, searcher_ast, rule_name)
    }

    // TODO: Could copy using apply_pat from Pattern impl for better efficiency. Need to make it public?
    fn apply_matches(
        &self,
        egraph: &mut EGraph<L, N>,
        matches: &[SearchMatches<L>],
        rule_name: Arc<str>,
    ) -> Vec<Id> {
        // let mut added = vec![]; // added are union updates, of which there are none
        for mat in matches {
            for subst in &mat.substs {
                self.apply_one(egraph, 0.into(), subst, None, rule_name.clone());
                // root is just ignored?
            }
        }
        // TODO: REALLY THINK ABOUT THIS!!!!
        vec![0.into()] // so a clause may not make more stuff happen. Early saturation.
    }

    fn vars(&self) -> Vec<egg::Var> {
        self.0.vars()
    }
}

type SymExpr = RecExpr<SymbolLang>;
type SymEGraph = EGraph<SymbolLang, ()>;

fn simplify(egraph: &SymEGraph, eid: Id) -> SymExpr {
    let extractor = Extractor::new(egraph, AstSize);
    let (_best_cost, best) = extractor.find_best(eid);
    best
}

fn print_subst<T: std::fmt::Write>(
    buf: &mut T,
    egraph: &EGraph<SymbolLang, ()>,
    subst: &Subst,
) -> Result<(), std::fmt::Error> {
    write!(buf, "[");
    let mut iter = subst.vec.iter();
    if let Some((k, eid)) = iter.next() {
        let best = simplify(egraph, *eid);
        write!(buf, "{} = {}", k, best)?;
        for (k, eid) in iter {
            let best = simplify(egraph, *eid);
            write!(buf, ", {} = {}", k, best)?;
        }
    }
    writeln!(buf, "];")
}

type SymMultiPattern = MultiPattern<EqWrap<Pattern<SymbolLang>>>;

// Current directory and already included set?
#[derive(Debug)]
pub struct Env {
    runner: Runner<SymbolLang, ()>,
    rules: Vec<egg::Rewrite<SymbolLang, ()>>,
    queries: Vec<MultiPattern<EqWrap<Pattern<SymbolLang>>>>,
}

impl Default for Env {
    fn default() -> Self {
        Env {
            runner: Runner::default(),
            queries: vec![],
            rules: vec![],
        }
    }
}

use std::fs;
/* // For use in the include directive
fn load_file(env: &mut Env, filename: &str) -> Result<(), String> {
    match fs::read_to_string(filename) {
        Err(e) => Err(format!("Error: file {} not found", filename)),
        Ok(contents) => match parse_file(contents) {
            Err(e) => Err(format!(
                "Error : file {} failed to parse with error : {}",
                filename, e
            )),
            Ok(entries) => {
                for entry in entries {
                    process_entry(env, entry);
                }
                Ok(())
            }
        },
    }
}
*/

use std::collections::HashSet;
#[derive(Debug, PartialEq, Clone)]
struct Env2 {
    freshvars: HashSet<String>, // forall x adds into this set
    metavars: HashSet<String>,  // exists x add into this set.
                                // entries : Vec<Entry>,
}

impl Env2 {
    fn new() -> Self {
        Env2 {
            freshvars: HashSet::default(),
            metavars: HashSet::default(),
        }
    }
}

fn interp_term(env: &Env2, t: &Term) -> Term {
    match t {
        Var(x) => panic!("Impossible"), // should parse formula at groundterms.
        Apply(f, args) => {
            if args.len() == 0 && env.freshvars.contains(f) {
                Var(f.clone())
            } else {
                Apply(
                    f.to_string(),
                    args.iter().map(|f2| interp_term(env, f2)).collect(),
                )
            }
        }
    }
}

fn interp_eqwrap(env: &Env2, t: &EqWrap<Term>) -> EqWrap<Term> {
    match t {
        Eq(a, b) => Eq(interp_term(env, a), interp_term(env, b)),
        Bare(a) => Bare(interp_term(env, a)),
    }
}

fn interp_term_goal(env: &Env2, t: &Term) -> Term {
    match t {
        Var(x) => panic!("Impossible"),
        Apply(f, args) => {
            if args.len() == 0 && env.metavars.contains(f) {
                Var(f.clone())
            } else {
                Apply(
                    f.to_string(),
                    args.iter().map(|f2| interp_term_goal(env, f2)).collect(),
                )
            }
        }
    }
}

fn interp_eqwrap_goal(env: &Env2, t: &EqWrap<Term>) -> EqWrap<Term> {
    match t {
        Eq(a, b) => Eq(interp_term_goal(env, a), interp_term_goal(env, b)),
        Bare(a) => Bare(interp_term_goal(env, a)),
    }
}
use Formula::*;
// We shouldn't be using mutable envs. What am I thinking?

/*
More imperative style to a program?
enum SearchProgram {
    Run,
    Clear,
}
*/

// Module?
#[derive(Debug, Clone)]
pub struct Program {
    // eqfacts and facts, or just duplicate for base facts?
    facts: Vec<(RecExpr<SymbolLang>, RecExpr<SymbolLang>)>,
    rules: Vec<egg::Rewrite<SymbolLang, ()>>,
    queries: Vec<MultiPattern<EqWrap<Pattern<SymbolLang>>>>,
}

impl Default for Program {
    fn default() -> Self {
        Program {
            facts: vec![],
            queries: vec![],
            rules: vec![],
        }
    }
}

pub fn process_entry_prog(prog: &mut Program, entry: Entry) {
    match entry {
        Directive(types::Directive::Include(filename)) => (), // load_file(state, &filename).unwrap(),
        Fact(Eq(a, b)) => {
            let a = recexpr_of_groundterm(&a);
            let b = recexpr_of_groundterm(&b);
            prog.facts.push((a, b))
        }
        Fact(Bare(a)) => {
            let a = recexpr_of_groundterm(&a);
            prog.facts.push((a.clone(), a))
        }
        Clause(head, body) => {
            let body = body
                .iter()
                .map(|eqt| match eqt {
                    Bare(p) => Bare(pattern_of_term(p)),
                    Eq(a, b) => Eq(pattern_of_term(a), pattern_of_term(b)),
                })
                .collect();
            let searcher = MultiPattern { patterns: body };
            let head = head
                .iter()
                .map(|eqt| match eqt {
                    Bare(p) => Bare(pattern_of_term(p)),
                    Eq(a, b) => Eq(pattern_of_term(a), pattern_of_term(b)),
                })
                .collect();
            let applier = MultiPattern { patterns: head };
            prog.rules.push(
                egg::Rewrite::new(format!("{}:-{}.", applier, searcher), searcher, applier)
                    .unwrap(),
            );
        }
        BiRewrite(a, b) => {
            let a = pattern_of_term(&a);
            let b = pattern_of_term(&b);
            prog.rules
                .push(egg::Rewrite::new(format!("{} -> {}", a, b), a.clone(), b.clone()).unwrap());
            prog.rules
                .push(egg::Rewrite::new(format!("{} -> {}", b, a), b, a).unwrap());
        }
        Rewrite(a, b, body) => {
            let a = pattern_of_term(&a);
            let b = pattern_of_term(&b);
            // consider shortcircuiting case where body = []
            let conditions: Vec<_> = body
                .iter()
                .map(|e| {
                    let (l, r) = match e {
                        Eq(a, b) => (pattern_of_term(&a), pattern_of_term(&b)),
                        Bare(a) => (pattern_of_term(&a), pattern_of_term(&a)),
                    };
                    ConditionEqual::new(l, r)
                })
                .collect();
            let condition = move |egraph: &mut EGraph<_, ()>, eclass: Id, subst: &Subst| {
                conditions.iter().all(|c| c.check(egraph, eclass, subst))
            };
            let applier = ConditionalApplier {
                condition,
                applier: a.clone(),
            };
            if body.len() == 0 {
                prog.rules
                    .push(egg::Rewrite::new(format!("{} -> {}", b, a), b, applier).unwrap());
            } else {
                prog.rules.push(
                    egg::Rewrite::new(format!("{} -{:?}> {}", b, body, a), b, applier).unwrap(),
                );
            }
        }
        Query(qs) => {
            let qs = qs.iter().map(pattern_of_eqterm).collect();
            prog.queries.push(MultiPattern { patterns: qs });
        }
        Axiom(_name, f) => interp_formula(prog, &mut Env2::new(), f), // I should use the name
        Goal(f) => interp_goal(prog, &mut Env2::new(), f),
    }
}

// run_program with default Runner
fn run_program2(prog: &Program) -> Vec<Vec<Subst>> {
    let runner = Runner::default()
        .with_iter_limit(30)
        .with_node_limit(10_000)
        .with_time_limit(Duration::from_secs(5));
    let (_runner, res) = run_program(prog, runner);
    res
}

fn run_program(
    prog: &Program,
    mut runner: Runner<SymbolLang, ()>,
) -> (Runner<SymbolLang, ()>, Vec<Vec<Subst>>) {
    let egraph = &mut runner.egraph;
    for (a, b) in &prog.facts {
        //let a_id = egraph.add_expr(&a);
        //let b_id = egraph.add_expr(&b);
        let a = a.to_string().parse().unwrap();
        let b = b.to_string().parse().unwrap();
        egraph.union_instantiations(&a, &b, &Subst::with_capacity(0), Arc::from("Base Fact"));
    }
    let runner = runner.run(&prog.rules);
    let res = prog
        .queries
        .iter()
        .map(|q| {
            let matches = q.search(&runner.egraph);
            matches.into_iter().flat_map(|mat| mat.substs).collect()
        })
        .collect();
    (runner, res)
}

use std::collections::HashMap;
fn freshen_formula(vs: Vec<String>, f: &Formula) -> Formula {
    let mut freshmap = HashMap::new();
    for v in &vs {
        freshmap.insert(v.clone(), gensym::gensym(&v));
    }
    fn freshen_term(freshmap: &HashMap<String, String>, vs: &Vec<String>, t: &Term) -> Term {
        match t {
            Var(x) => Var(x.clone()),
            Apply(f, args) => {
                if args.len() == 0 {
                    if vs.contains(f) {
                        Apply(freshmap.get(f).unwrap().clone(), vec![])
                    } else {
                        Apply(f.clone(), vec![])
                    }
                } else {
                    Apply(
                        f.clone(),
                        args.iter()
                            .map(|arg| freshen_term(freshmap, vs, arg))
                            .collect(),
                    )
                }
            }
        }
    }
    fn worker(fm: &HashMap<String, String>, vs: &Vec<String>, f: &Formula) -> Formula {
        if vs.len() == 0 {
            f.clone()
        } else {
            match f {
                Impl(hyp, conc) => Impl(
                    Box::new(worker(fm, vs, &*hyp)),
                    Box::new(worker(fm, vs, &*conc)),
                ),
                Conj(fs) => Conj(fs.iter().map(|f| worker(fm, vs, f)).collect()),
                Disj(fs) => Disj(fs.iter().map(|f| worker(fm, vs, f)).collect()),
                ForAll(vs2, f) => {
                    let mut vs = vs.clone();
                    vs.retain(|v| !vs2.contains(v));
                    ForAll(vs2.clone(), Box::new(worker(fm, &vs, &*f)))
                }
                Exists(vs2, f) => {
                    let mut vs = vs.clone();
                    vs.retain(|v| !vs2.contains(v));
                    Exists(vs2.clone(), Box::new(worker(fm, &vs, &*f)))
                }
                Atom(t) => Atom(match t {
                    Eq(a, b) => Eq(freshen_term(fm, vs, a), freshen_term(fm, vs, b)),
                    Bare(a) => Bare(freshen_term(fm, vs, a)),
                }),
            }
        }
    }
    worker(&freshmap, &vs, f)
}

fn interp_goal(prog: &mut Program, env: &Env2, formula: Formula) {
    match formula {
        Conj(fs) => {
            let ps = fs
                .iter()
                .map(|g| match g {
                    // TODO: How to not insist on Atom here?
                    // recurse on fresh progs, accumulate all queries into single query.
                    // (sum of products)
                    Atom(g) => pattern_of_eqterm(&interp_eqwrap_goal(env, g)),
                    _ => panic!("unexpected form in goal"),
                })
                .collect();
            prog.queries.push(MultiPattern { patterns: ps })
        }
        Atom(f) => {
            let g = MultiPattern {
                patterns: vec![pattern_of_eqterm(&interp_eqwrap_goal(env, &f))],
            };
            prog.queries.push(g)
        }
        Exists(vs, f) => {
            let mut env = env.clone();
            // freshen?
            env.metavars.extend(vs); // patvars?
            interp_goal(prog, &env, *f)
        }
        //Impl(hyp, conc) => {
            //Is this right? Aren't I injecting hypotheses into the program for other queries then?
            // These hypotheses need to be retracted.
        //    interp_formula(prog, env, *hyp);
        //    interp_goal(prog, env, *conc);
        //}
        ForAll(vs, f) => {
            let f = freshen_formula(vs, &*f);
            // freshvars mapping rather than doing eager freshen?
            interp_goal(prog, env, f);
        }
        _ => panic!("no other goal"),
    }
}

// I should make env immutable this is not right as is.
// Or what is even the point of being this fancy
// Could I implement higher order rules by extending symbollang?
/*
P => (Q => R) becomes
P => x
x /\ Q => R
P => (Q => R) should really be P /\ Q => R ?

(P => Q) => R


*/
struct Sequent {
    hyps: Vec<Formula>,
    conc: Formula, // sig : Vec<String>
}
/*
The only obligations that can be discharged via egglog are those of the form
---------------------------------------- (egglog)
atom, atom, hyp => conc, hyp => conc |- conj(q1,q2,q3), conj()
which is indeed the form of a "Program".

All other transformations should be recorded as an internal proof tree.

enum Proof {
    LAnd(Int, Box<Proof>, Box<Proof>),


}
*/
/*
fn proof(s : Sequent) -> Vec<Program>, Proof {
    match s.conc {
        Conj(fs) => fs.map(|conc| proof(Sequent{ s.hyps.clone(), conc})),
        Disj(_) => fs.map(|conc| ),
        ForAll(vs,f) => proof(hyps, freshen_formula(vs, f))
        Impl(hyp,conc) => proof(hyps.push(hyp), conc )
        Atom() => run_query( hyps, q )
    }
}



fn run_query(hyps, q) {
    prog = Program::default()
    while let Some(hyp) = hyps.pop() {

    }
    for hyp in hyps {
        match hyp {
            Atom(f) => prog.facts.push(f)
            Conj(fs) => hyps.extend(f)  // Left And
            ForAll(vs, f) => patvarize(vs, f) //| is_prim_rewrite(f) =>
                          //| otherwise =>
            Impl(a,b) | is_prim_pat(a) && is_prim_applier(b) => ReWrite
            Impl(a,b) => proof( other_hyps, a ) and proof(hyps + b, q)


        }
    }
}
*/

fn interp_formula(prog: &mut Program, env: &Env2, formula: Formula) {
    match formula {
        Atom(a) => { // I can support forall x, f x = g x as a bidirectional rule
            match interp_eqwrap(env, &a) {
                Eq(a, b) => {
                    let a = recexpr_of_groundterm(&is_ground(&a).unwrap());
                    let b = recexpr_of_groundterm(&is_ground(&b).unwrap());
                    prog.facts.push((a, b))
                }
                Bare(a) => {
                    let a = recexpr_of_groundterm(&is_ground(&a).unwrap());
                    prog.facts.push((a.clone(), a))
                }
            };
        }
        Conj(fs) => {
            for f in fs {
                interp_formula(prog, env, f);
            }
        }
        Impl(hyp, conc) => {
            /*
            prog = Program::defualt();
            let prog = interp_goal( prog, env, &hyp  );
            assert prog.clauses == 0
            assert prog.facts == 0
            assert prog.queries.len() == 1
            Rewrite(prog.queries, prog.
            */
            let hyps = match *hyp {
                Atom(hyp) => vec![pattern_of_eqterm(&interp_eqwrap(env, &hyp))],
                Conj(hyps) => hyps
                    .iter()
                    .map(|hyp| match hyp {
                        Atom(hyp) => pattern_of_eqterm(&interp_eqwrap(env, hyp)),
                        _ => panic!("invalid hyp in conj"),
                    })
                    .collect(),
                _ => panic!("Invalid hyp {:?}", *hyp),
            };
            // I should be not duplicating code here.
            // call interp_formula here. assert no queries only facts?
            let concs = match *conc {
                Atom(conc) => vec![pattern_of_eqterm(&interp_eqwrap(env, &conc))],
                Conj(concs) => concs
                    .iter()
                    .map(|conc| match conc {
                        Atom(conc) => pattern_of_eqterm(&interp_eqwrap(env, conc)),
                        _ => panic!("invalid conc in conj"),
                    })
                    .collect(),
                _ => panic!("Invalid conc {:?}", *conc),
            };
            let searcher = MultiPattern { patterns: hyps };
            let applier = MultiPattern { patterns: concs };
            prog.rules
                .push(egg::Rewrite::new("", searcher, applier).unwrap())
        }
        // Exists in conclusion. Skolemized on freshvars?
        // We can't allow unguarded exists though. uh. Yes we can.
        //Exists(vs, f) => {
            // freshvars is a bad name
       //     let freshvars = vs.map(|v| Apply(gensym(v), env.freshvars );

        //}
        // Nested Programs? swaping facts and queries in some sense?
        ForAll(vs, f) => {
            let mut env = env.clone();
            env.freshvars.extend(vs); // patvars?
            interp_formula(prog, &env, *f)
        }
        _ => panic!("unexpected formula {:?} in interp_formula", formula),
    }
}
/*

// allowing P /\ A => B in head of rules would be useful for exists_unique.


// This vs implementing searcher for a formula itself.
fn interp_searcher(formula : Formula) -> impl Searcher {
    match formula {
        Conj(xs) => ,
        Atom(Eq(a,b)) =>,
        Atom(Bare(a)) =>,
        _ => panic
    }

}

// Alt pattern implements "Or" search.
// It should run each of it's searchers and collate the results.
// Unlike MultiPattern it make no sense as an Applier, since we don't know which case to use.
struct AltPattern<P>{
    pats : Vec<P>
}

// Only succeeds if P fails.
struct NegPattern<P>{
    pat : P
}

*/

fn apply_subst(
    pat: &PatternAst<SymbolLang>,
    subst: &Subst,
    egraph: &EGraph<SymbolLang, ()>,
) -> RecExpr<SymbolLang> {
    //let mut r = RecExpr::default();
    //for (i, pat_node) in pat.iter().enumerate() {
    fn worker(
        i: Id,
        pat: &PatternAst<SymbolLang>,
        subst: &Subst,
        egraph: &EGraph<SymbolLang, ()>,
    ) -> String {
        match &pat[i] {
            ENodeOrVar::Var(w) => {
                //let expr = simplify(egraph,subst[*w] );
                // r.add(egraph.extract(subst[*w]))
                simplify(egraph, *subst.get(*w).unwrap()).to_string()
            }
            ENodeOrVar::ENode(e) => {
                let args: String = e
                    .children
                    .iter()
                    .map(|i| worker(*i, pat, subst, egraph))
                    .collect();
                format!("({} {})", e.op.to_string(), args)
                //let n = e.clone().map_children(|child| ids[usize::from(child)]);
                //egraph.add(n)
            }
        }
    }
    worker((pat.as_ref().len() - 1).into(), pat, subst, egraph)
        .parse()
        .unwrap()
}

use core::time::Duration;
// Refactor this to return not string.
fn run_file(file: Vec<Entry>, opts: &Opts) -> String {
    //let mut env = Env::default();
    let mut prog = Program::default();

    for entry in file {
        //process_entry(&mut env, entry)
        process_entry_prog(&mut prog, entry)
    }
    let runner = Runner::default()
        .with_iter_limit(30)
        .with_node_limit(10_000)
        .with_time_limit(Duration::from_secs(5))
        .with_explanations_enabled();
    let (mut runner, query_results) = run_program(&prog, runner);
    // Two useful things to turn on. Command line arguments?
    //runner.print_report();
    // runner.egraph.dot().to_png("target/foo.png").unwrap();
    let mut buf = String::new();
    for (q, res) in prog.queries.iter().zip(query_results) {
        writeln!(buf, "-? {}", q);
        //let matches = q.search(&runner.egraph);
        if res.len() == 0 {
            writeln!(buf, "unknown.");
        } else {
            for subst in res {
                print_subst(&mut buf, &runner.egraph, &subst);
                if opts.proof {
                    for ab in &q.patterns {
                        if let Eq(a, b) = ab {
                            /*
                            let ast = a.ast.as_ref();
                            let mut id_buf = vec![0.into(); ast.len()];
                            let start = egg::pattern::apply_pat(&mut id_buf, ast, &mut runner.egraph, &subst);
                            let start = simplify(&runner.egraph, start);

                            let ast = b.ast.as_ref();
                            let mut id_buf = vec![0.into(); ast.len()];
                            let end = egg::pattern::apply_pat(&mut id_buf, ast, &mut runner.egraph, &subst);
                            let end = simplify(&runner.egraph, end);
                            */
                            let start = apply_subst(&a.ast, &subst, &runner.egraph);
                            let end = apply_subst(&b.ast, &subst, &runner.egraph);
                            writeln!(
                                buf,
                                "Proof {} = {}: {}",
                                a,
                                b,
                                runner.explain_equivalence(&start, &end).get_flat_string()
                            );
                        }
                    }
                }
            }
        }
    }
    buf
}

use clap::{AppSettings, Clap};

/// A Prolog-like theorem prover based on Egg
#[derive(Clap)]
#[clap(version = "0.01", author = "Philip Zucker <philzook58@gmail.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Opts {
    /// Path of Egglog file to run
    pub filename: Option<String>,
    /// Turn off verbosity TODO
    #[clap(short, long)]
    pub verbose: bool, // quiet?
    /// Enable Proof Generation (Experimental)
    #[clap(short, long)]
    pub proof: bool, // quiet?
    /// Output graphical representation TODO
    #[clap(short, long)]
    pub graph: Option<String>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            filename: None,
            verbose: false,
            proof: false,
            graph: None,
        }
    }
}

pub fn run(s: String, opts: &Opts) -> Result<String, String> {
    let f = parse_file(s)?;
    Ok(run_file(f, opts))
}

use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub fn run_wasm_simple(s: String) -> String {
    let opts = Opts::default();
    match run(s, &opts) {
        Ok(e) => e,
        Err(e) => e,
    }
}

#[wasm_bindgen]
pub fn run_wasm(s: String, proof: bool, graph: bool) -> String {
    let opts = Opts::default();
    let opts = Opts {
        proof,
        //graph : if graph {Some "graphout.viz"} else None,
        ..opts
    };
    match run(s, &opts) {
        Ok(e) => e,
        Err(e) => e,
    }
}
