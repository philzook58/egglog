use egg::*;
use std::fmt;

#[derive(Debug, PartialEq, Clone)]
pub enum Term {
    Var(String),
    Apply(String, Vec<Term>),
}
use Term::*;

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Var(v) => write!(f, "?{}", v),

            Apply(g, args) => {
                write!(f, "{}(", g)?;
                for a in args {
                    write!(f, "{},", a)?;
                }
                write!(f, ")")
            }
        }
    }
}

// There is an argument to be made that I should directly be using RecExpr for groundterm and Pattern for Term
#[derive(Debug, PartialEq)]
pub struct GroundTerm {
    pub head: String,
    pub args: Vec<GroundTerm>,
}

impl fmt::Display for GroundTerm {
    fn fmt(&self, buf: &mut fmt::Formatter) -> fmt::Result {
        if self.args.len() == 0 {
            write!(buf, "{}", self.head)
        } else {
            write!(buf, "{}(", self.head)?;
            let mut iter = self.args.iter();
            if let Some(item) = iter.next() {
                write!(buf, "{}", item)?;
                for item in iter {
                    write!(buf, ", {}", item)?;
                }
            }
            write!(buf, ")")
        }
    }
}

// toplevel of term is eq only
#[derive(Debug, PartialEq, Clone)]
pub enum EqWrap<T> {
    Eq(T, T),
    Bare(T),
}
// We could also consider MultiEq { terms : Vec<T> } for more than one A= B = C = D

impl<T> EqWrap<T> {
    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> EqWrap<U> {
        match self {
            EqWrap::Bare(x) => EqWrap::Bare(f(x)),
            EqWrap::Eq(a, b) => EqWrap::Eq(f(a), f(b)),
        }
    }
}

impl<T> fmt::Display for EqWrap<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EqWrap::Eq(a, b) => write!(f, "{} = {}", a, b),
            EqWrap::Bare(v) => write!(f, "{}", v),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Directive {
    Include(String),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Formula {
    Impl(Box<Formula>, Box<Formula>),
    Conj(Vec<Formula>),
    Disj(Vec<Formula>),
    ForAll(Vec<String>, Box<Formula>),
    Exists(Vec<String>, Box<Formula>),
    Atom(EqWrap<Term>),
}

#[derive(Debug, PartialEq)]
pub enum Entry {
    Clause(Vec<EqWrap<Term>>, Vec<EqWrap<Term>>),
    Fact(EqWrap<GroundTerm>),
    Rewrite(Term, Term, Vec<EqWrap<Term>>),
    BiRewrite(Term, Term),
    Directive(Directive),
    Query(Vec<EqWrap<Term>>), // Should I only allow GroundTerm queries?
    Axiom(String, Formula),
    Goal(Formula),
}

/* enum Directive {
NodeLimit,
ClassLimit
TimeLimit,
Include,
Clear/Reset
}

*/
pub fn is_ground(t: &Term) -> Option<GroundTerm> {
    match t {
        Var(_) => None,
        Apply(f, args) => {
            let oargs: Option<Vec<GroundTerm>> = args.iter().map(is_ground).collect();
            oargs.map(|args| GroundTerm {
                head: f.to_string(),
                args,
            })
        }
    }
}

pub fn eid_of_groundterm(egraph: &mut EGraph<SymbolLang, ()>, t: &GroundTerm) -> Id {
    let args = t
        .args
        .iter()
        .map(|a| eid_of_groundterm(egraph, a))
        .collect();
    egraph.add(SymbolLang::new(t.head.clone(), args))
}

fn recexpr_of_groundterm_aux(expr: &mut RecExpr<SymbolLang>, t: &GroundTerm) -> Id {
    let expr_args = t
        .args
        .iter()
        .map(|a| recexpr_of_groundterm_aux(expr, &a))
        .collect();
    expr.add(SymbolLang::new(t.head.clone(), expr_args))
}

pub fn recexpr_of_groundterm(t: &GroundTerm) -> RecExpr<SymbolLang> {
    let mut expr = RecExpr::default();
    recexpr_of_groundterm_aux(&mut expr, t);
    expr
}

pub fn pattern_of_eqterm(t: &EqWrap<Term>) -> EqWrap<Pattern<SymbolLang>> {
    match t {
        EqWrap::Bare(x) => EqWrap::Bare(pattern_of_term(x)),
        EqWrap::Eq(x, y) => EqWrap::Eq(pattern_of_term(x), pattern_of_term(y)),
    }
}
/*
fn pattern_of_term(t : &Term) -> Pattern<SymbolLang> {
    let mut ast = RecExpr::default();
    fn worker(t : &Term){
        match t {
            Var(x) => ast.add(ENodeOrVar::Var(Var(Symbol::from("x"))))
            Apply(f,args) =>
              let args = args.iter().map(worker).collect();
             ast.add(ENodeOrVar::ENode( SymbolLang::new(f.clone(),args)))
        }
    }
    worker(t);
    let program = egg::machine::Program::compile_from_pat(&ast);
    Pattern { ast, program }
}
*/
pub fn sexp_of_term(t: &Term) -> String {
    match t {
        Var(x) => format!(" ?{} ", x),
        Apply(f, args) => {
            let args: String = args.iter().map(sexp_of_term).collect();
            format!("({}{})", f, args)
        }
    }
}

// This sort of stuff is what From traits are for right?
pub fn pattern_of_term(t: &Term) -> Pattern<SymbolLang> {
    sexp_of_term(t).parse().unwrap()
}

/*

*/
/*
// Private. options ;
// 1 trasmute the memory. Yikes.
// 2 Rebuild the machine infrastructure in a file here. Compile a single machine that produces a single subst.
// 3 Fork egg
fn merge_subst( s1 : &Subst, s2 : &Subst ) -> Option<Subst>{
    let s1 = s1.clone();
    for (v,i) in s2.vec {
        if let Some(id) = s1.insert(v,i){
            return None;
        }
    }
    return Some(s1);
}
*/
