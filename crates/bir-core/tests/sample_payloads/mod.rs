//! Add `mod form_<code>;` here for each form with a sample payload, and its
//! `agent_seeds` to [`agent_seeds`].

mod agent_forms;

pub(crate) mod form_0605;
pub(crate) mod form_0619e;
pub(crate) mod form_0619f;
pub(crate) mod form_1600pt;
pub(crate) mod form_1600vt;
pub(crate) mod form_1600wp;
pub(crate) mod form_1601c;
pub(crate) mod form_1601eq;
pub(crate) mod form_1601fq;
pub(crate) mod form_1602q;
pub(crate) mod form_1603q;
pub(crate) mod form_1604c;
pub(crate) mod form_1604e;
pub(crate) mod form_1604f;
pub(crate) mod form_1606;
pub(crate) mod form_1700;
pub(crate) mod form_1701;
pub(crate) mod form_1701a;
pub(crate) mod form_1701ms;
pub(crate) mod form_1701q;
pub(crate) mod form_1702ex;
pub(crate) mod form_1702mx;
pub(crate) mod form_1702q;
pub(crate) mod form_1702rt;
pub(crate) mod form_1706;
pub(crate) mod form_1707;
pub(crate) mod form_1707a;
pub(crate) mod form_1800;
pub(crate) mod form_1801;
pub(crate) mod form_2000;
pub(crate) mod form_2000ot;
pub(crate) mod form_2200a;
pub(crate) mod form_2200an;
pub(crate) mod form_2200c;
pub(crate) mod form_2200m;
pub(crate) mod form_2200p;
pub(crate) mod form_2200s;
pub(crate) mod form_2200t;
pub(crate) mod form_2550m;
pub(crate) mod form_2550q;
pub(crate) mod form_2551q;
pub(crate) mod form_2552;
pub(crate) mod form_2553;
pub mod penalty_items;
mod rendered_print_parity;
// every other form keeps its penalty due dates and amounts.
mod penalty_golden;

/// Sample drafts per form module, for the agent-form probe.
pub(crate) fn agent_seeds() -> Vec<(&'static str, Vec<serde_json::Value>)> {
    vec![
        ("0605", form_0605::agent_seeds()),
        ("0619e", form_0619e::agent_seeds()),
        ("0619f", form_0619f::agent_seeds()),
        ("1600pt", form_1600pt::agent_seeds()),
        ("1600vt", form_1600vt::agent_seeds()),
        ("1600wp", form_1600wp::agent_seeds()),
        ("1601c", form_1601c::agent_seeds()),
        ("1601eq", form_1601eq::agent_seeds()),
        ("1601fq", form_1601fq::agent_seeds()),
        ("1602q", form_1602q::agent_seeds()),
        ("1603q", form_1603q::agent_seeds()),
        ("1604c", form_1604c::agent_seeds()),
        ("1604e", form_1604e::agent_seeds()),
        ("1604f", form_1604f::agent_seeds()),
        ("1606", form_1606::agent_seeds()),
        ("1700", form_1700::agent_seeds()),
        ("1701", form_1701::agent_seeds()),
        ("1701a", form_1701a::agent_seeds()),
        ("1701ms", form_1701ms::agent_seeds()),
        ("1701q", form_1701q::agent_seeds()),
        ("1702ex", form_1702ex::agent_seeds()),
        ("1702mx", form_1702mx::agent_seeds()),
        ("1702q", form_1702q::agent_seeds()),
        ("1702rt", form_1702rt::agent_seeds()),
        ("1706", form_1706::agent_seeds()),
        ("1707", form_1707::agent_seeds()),
        ("1707a", form_1707a::agent_seeds()),
        ("1800", form_1800::agent_seeds()),
        ("1801", form_1801::agent_seeds()),
        ("2000", form_2000::agent_seeds()),
        ("2000ot", form_2000ot::agent_seeds()),
        ("2200a", form_2200a::agent_seeds()),
        ("2200an", form_2200an::agent_seeds()),
        ("2200c", form_2200c::agent_seeds()),
        ("2200m", form_2200m::agent_seeds()),
        ("2200p", form_2200p::agent_seeds()),
        ("2200s", form_2200s::agent_seeds()),
        ("2200t", form_2200t::agent_seeds()),
        ("2550m", form_2550m::agent_seeds()),
        ("2550q", form_2550q::agent_seeds()),
        ("2551q", form_2551q::agent_seeds()),
        ("2552", form_2552::agent_seeds()),
        ("2553", form_2553::agent_seeds()),
    ]
}
