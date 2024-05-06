//! Simple graphviz dot file format output.

use std::fmt::{self, Display, Write};

use bitflags::bitflags;

use crate::exec_graph::{Direction, EdgeIndex, ExecGraph, NodeIndex, NodeType};

/// `Dot` implements output to graphviz .dot format for a graph.
///
/// Formatting and options are rather simple, this is mostly intended
/// for debugging. Exact output may change.
///
/// # Examples
///
/// ```
/// use petgraph::Graph;
/// use petgraph::dot::{Dot, Config};
///
/// let mut graph = Graph::<_, ()>::new();
/// graph.add_node("A");
/// graph.add_node("B");
/// graph.add_node("C");
/// graph.add_node("D");
/// graph.extend_with_edges(&[
///     (0, 1), (0, 2), (0, 3),
///     (1, 2), (1, 3),
///     (2, 3),
/// ]);
///
/// println!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
///
/// // In this case the output looks like this:
/// //
/// // digraph {
/// //     0 [label="\"A\""];
/// //     1 [label="\"B\""];
/// //     2 [label="\"C\""];
/// //     3 [label="\"D\""];
/// //     0 -> 1;
/// //     0 -> 2;
/// //     0 -> 3;
/// //     1 -> 2;
/// //     1 -> 3;
/// //     2 -> 3;
/// // }
///
/// // If you need multiple config options, just list them all in the slice.
/// ```
pub struct Dot<'a> {
    graph: &'a ExecGraph,
    config: Config,
    get_edge_attributes: &'a dyn Fn(&ExecGraph, EdgeIndex) -> String,
    get_node_attributes: &'a dyn Fn(&ExecGraph, NodeIndex) -> String,
}

static TYPE: [&str; 2] = ["graph", "digraph"];
static EDGE: [&str; 2] = ["--", "->"];
static INDENT: &str = "\t";

impl<'a> Dot<'a> {
    /// Create a `Dot` formatting wrapper with default configuration.
    #[inline]
    pub fn new(graph: &'a ExecGraph) -> Self {
        Self::with_config(graph, Config::empty())
    }

    /// Create a `Dot` formatting wrapper with custom configuration.
    #[inline]
    pub fn with_config(graph: &'a ExecGraph, config: Config) -> Self {
        Self::with_attr_getters(graph, config, &|_, _| String::new(), &|_, _| String::new())
    }

    #[inline]
    pub fn with_attr_getters(
        graph: &'a ExecGraph,
        config: Config,
        get_edge_attributes: &'a dyn Fn(&ExecGraph, EdgeIndex) -> String,
        get_node_attributes: &'a dyn Fn(&ExecGraph, NodeIndex) -> String,
    ) -> Self {
        Dot {
            graph,
            config,
            get_edge_attributes,
            get_node_attributes,
        }
    }
}

// `Dot` configuration.
bitflags! {
    pub struct Config: u32 {
        const NODE_NO_LABEL = 0b001;
        const EDGE_NO_LABEL = 0b010;
        const GRAPH_CONTENT_ONLY = 0b100;
    }
}
impl<'a> Dot<'a> {
    fn graph_fmt<NF>(&self, f: &mut fmt::Formatter, node_fmt: NF) -> fmt::Result
    where
        NF: Fn(&NodeType, &mut fmt::Formatter) -> fmt::Result,
    {
        let g = &self.graph;
        if !self.config.contains(Config::GRAPH_CONTENT_ONLY) {
            writeln!(f, "{} {{", TYPE[1])?;
        }

        // for i in g.node_references() {
        //     if let crate::exec_graph::NodeType::System(_,_ ) = i.label() {
        //         println!("g.node_references()============={:?}", i);
        //     }
            
        // }
        
        // output all labels
        for (index, node) in g.node_references().enumerate() {
            write!(f, "{}{}", INDENT, index,)?;
            if !self.config.contains(Config::NODE_NO_LABEL) {
                write!(f, " [label=\"")?;
                Escaped(FnFmt(node.label(), &node_fmt)).fmt(f)?;
                write!(f, "\"")?;
                let s = (self.get_node_attributes)(g, NodeIndex::new(index));
                if !s.is_empty() {
                    write!(f, ", {}", s)?;
                }
                writeln!(f, "];")?;
            }
        }
        // output all edges
        for (index, edge) in g.edge_references().enumerate() {
            let from = edge.load(Direction::From).0;
            let to = edge.load(Direction::To).0;
            write!(f, "{}{} {} {}", INDENT, from.index(), EDGE[1], to.index(),)?;
            if self.config.contains(Config::EDGE_NO_LABEL) {
                let s = (self.get_edge_attributes)(g, EdgeIndex::new(index));
                if !s.is_empty() {
                    write!(f, "[{}]", s)?;
                }
            }
            writeln!(f, ";")?;
        }

        if !self.config.contains(Config::GRAPH_CONTENT_ONLY) {
            writeln!(f, "}}")?;
        }
        Ok(())
    }
}

impl<'a> fmt::Display for Dot<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.graph_fmt(f, fmt::Display::fmt)
    }
}

impl<'a> fmt::Debug for Dot<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.graph_fmt(f, fmt::Debug::fmt)
    }
}

/// Escape for Graphviz
struct Escaper<W>(W);

impl<W> fmt::Write for Escaper<W>
where
    W: fmt::Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c)?;
        }
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        match c {
            '"' | '\\' => self.0.write_char('\\')?,
            // \l is for left justified linebreak
            '\n' => return self.0.write_str("\\l"),
            _ => {}
        }
        self.0.write_char(c)
    }
}

/// Pass Display formatting through a simple escaping filter
struct Escaped<T>(T);

impl<T> fmt::Display for Escaped<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            writeln!(&mut Escaper(f), "{:#}", &self.0)
        } else {
            write!(&mut Escaper(f), "{}", &self.0)
        }
    }
}

/// Format data using a specific format function
struct FnFmt<'a, T, F>(&'a T, F);

impl<'a, T, F> fmt::Display for FnFmt<'a, T, F>
where
    F: Fn(&'a T, &mut fmt::Formatter<'_>) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.1(self.0, f)
    }
}
