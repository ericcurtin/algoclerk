// SPDX-License-Identifier: Apache-2.0
//! Member function calls: dispatch, strings and optionals (containers live in `seqs` and `assoc`).

use super::rval::*;
use super::{Loc, Machine, R, RtKind, Rv};
use crate::cpp::ast::*;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum RK {
    Str,
    View,
    Vec,
    Deque,
    List,
    Pq,
    Map,
    Set,
    UMap,
    USet,
    Opt,
}

pub(super) const NPOS: u64 = u64::MAX;

fn iter_name(name: &str) -> Option<(bool, bool)> {
    Some(match name {
        "begin" | "cbegin" => (false, false),
        "end" | "cend" => (true, false),
        "rbegin" | "crbegin" => (false, true),
        "rend" | "crend" => (true, true),
        _ => return None,
    })
}

impl<'p> Machine<'p> {
    pub(super) fn call_method(
        &mut self,
        e: &'p Expr,
        obj: &'p Expr,
        name: &'p str,
        arrow: bool,
        args: &'p [Expr],
    ) -> R<Rv> {
        let rp = if arrow {
            let rv = self.eval_rv(obj)?;
            self.deref_rv(rv)?
        } else {
            let rv = self.eval_rv(obj)?;
            self.rv_place(rv)
        };
        let kind = match self.get(&rp).map_err(|f| self.fail(f))? {
            Value::Str(_) => RK::Str,
            Value::View(_) => RK::View,
            Value::Vector(_) => RK::Vec,
            Value::Deque(_) => RK::Deque,
            Value::List(_) => RK::List,
            Value::Pq(_) => RK::Pq,
            Value::Map(_) => RK::Map,
            Value::Set(_) => RK::Set,
            Value::UMap(_) => RK::UMap,
            Value::USet(_) => RK::USet,
            Value::Opt(_) => RK::Opt,
            Value::Uninit(t) => {
                return Err(self.err(
                    RtKind::Uninit,
                    format!(
                        "this {t} was never set, so calling `{name}` on it is undefined behaviour"
                    ),
                ));
            }
            Value::Closure(_) | Value::FnRef(_) if name == "operator()" => {
                let f = self.load(&rp).map_err(|f| self.fail(f))?;
                return self.call_callable_args(&f, args);
            }
            _ => {
                return Err(self.err(
                    RtKind::BadArg,
                    format!("this value has no member function `{name}`"),
                ));
            }
        };
        if let Some((end, rev)) = iter_name(name) {
            if !args.is_empty() {
                return Err(self.err(RtKind::BadArg, "begin() and end() take no arguments"));
            }
            return Ok(Rv::Val(Value::Iter(Box::new(
                self.make_iter(&rp, end, rev)?,
            ))));
        }
        let _ = e;
        match kind {
            RK::Str | RK::View => self.m_str(rp, kind, name, args),
            RK::Vec => self.m_vec(rp, name, args),
            RK::Deque => self.m_deque(rp, name, args),
            RK::List => self.m_list(rp, name, args),
            RK::Pq => self.m_pq(rp, name, args),
            RK::Map | RK::Set => self.m_ord(rp, kind == RK::Map, name, args),
            RK::UMap | RK::USet => self.m_unord(rp, kind == RK::UMap, name, args),
            RK::Opt => self.m_opt(rp, name, args),
        }
    }

    fn call_callable_args(&mut self, f: &Value, args: &'p [Expr]) -> R<Rv> {
        let mut rvs = Vec::with_capacity(args.len());
        for a in args {
            rvs.push(self.eval_rv(a)?);
        }
        self.call_value(f, rvs)
    }

    // ----- argument helpers -----

    pub(super) fn nargs(&self, name: &str, args: &[Expr], lo: usize, hi: usize) -> R<()> {
        if args.len() < lo || args.len() > hi {
            return Err(self.err(
                RtKind::BadArg,
                format!("`{name}` does not take {} arguments", args.len()),
            ));
        }
        Ok(())
    }

    pub(super) fn arg_int(&mut self, a: &'p Expr) -> R<i128> {
        let v = self.eval(a)?;
        let v = self.check_init_val(v)?;
        v.as_i128()
            .ok_or_else(|| self.err(RtKind::BadArg, "this argument must be a number"))
    }

    /// An argument as a position or count: negative values wrap to huge unsigned numbers like size_t.
    pub(super) fn arg_size(&mut self, a: &'p Expr) -> R<u64> {
        let x = self.arg_int(a)?;
        Ok(x as u64)
    }

    pub(super) fn arg_iter(&mut self, a: &'p Expr) -> R<IterV> {
        match self.eval(a)? {
            Value::Iter(b) => Ok(*b),
            _ => Err(self.err(RtKind::BadArg, "this argument must be an iterator")),
        }
    }

    /// Bytes of a string-like argument (string, view, literal or a single character).
    pub(super) fn arg_bytes(&mut self, a: &'p Expr) -> R<Vec<u8>> {
        let rv = self.eval_rv(a)?;
        let v = match &rv {
            Rv::Val(v) => v.clone(),
            Rv::Place(p) => match self.loc(p).map_err(|f| self.fail(f))? {
                Loc::V(v) => v.clone(),
                Loc::B(b) => Value::Char(b as i8),
            },
        };
        match &v {
            Value::Char(c) => Ok(vec![*c as u8]),
            v => v.bytes().map(<[u8]>::to_vec).ok_or_else(|| {
                self.err(
                    RtKind::BadArg,
                    "this argument must be a string or a character",
                )
            }),
        }
    }

    pub(super) fn ret_size(n: usize) -> R<Rv> {
        Ok(Rv::Val(Value::Size(n as u64)))
    }

    fn pos_err(&self, what: &str) -> Box<super::RtError> {
        self.err(
            RtKind::Index,
            format!(
                "{what}: the position is past the end of the string (this would throw out_of_range)"
            ),
        )
    }

    // ----- strings -----

    fn str_bytes(&self, rp: &Place) -> R<Vec<u8>> {
        match self.get(rp).map_err(|f| self.fail(f))? {
            Value::Str(s) => Ok(s.s.clone()),
            Value::View(b) => Ok((**b).clone()),
            _ => Err(self.internal("not a string")),
        }
    }

    fn str_ref(&self, rp: &Place) -> R<&[u8]> {
        match self.get(rp).map_err(|f| self.fail(f))? {
            Value::Str(s) => Ok(&s.s),
            Value::View(b) => Ok(b),
            _ => Err(self.internal("not a string")),
        }
    }

    fn str_mut(&mut self, rp: &Place, bump: bool) -> R<&mut StrV> {
        let r = self.err(RtKind::BadArg, "a string_view cannot be modified this way");
        match self.get_mut(rp)? {
            Value::Str(s) => {
                if bump {
                    s.ver = s.ver.wrapping_add(1);
                }
                Ok(s)
            }
            _ => Err(r),
        }
    }

    fn m_str(&mut self, rp: Place, kind: RK, name: &'p str, args: &'p [Expr]) -> R<Rv> {
        let is_view = kind == RK::View;
        match name {
            "size" | "length" => {
                self.nargs(name, args, 0, 0)?;
                Self::ret_size(self.str_ref(&rp)?.len())
            }
            "empty" => Ok(Rv::Val(Value::Bool(self.str_ref(&rp)?.is_empty()))),
            "clear" => {
                let n = self.str_ref(&rp)?.len() as u64;
                self.charge(n)?;
                self.str_mut(&rp, true)?.s.clear();
                Ok(Rv::Val(Value::Void))
            }
            "push_back" => {
                let c = self.arg_int(&args[0])?;
                self.str_mut(&rp, true)?.s.push(c as u8);
                Ok(Rv::Val(Value::Void))
            }
            "pop_back" => {
                if self.str_ref(&rp)?.is_empty() {
                    return Err(self.err(RtKind::Empty, "pop_back() on an empty string"));
                }
                self.str_mut(&rp, true)?.s.pop();
                Ok(Rv::Val(Value::Void))
            }
            "back" | "front" => {
                let (len, ver) = self.str_len_ver(&rp)?;
                if len == 0 {
                    return Err(self.err(RtKind::Empty, format!("{name}() on an empty string")));
                }
                let i = if name == "back" { len - 1 } else { 0 };
                Ok(Rv::Place(rp.with(Step::Idx(i, ver))))
            }
            "at" => {
                self.nargs(name, args, 1, 1)?;
                let i = self.arg_int(&args[0])?;
                let (len, ver) = self.str_len_ver(&rp)?;
                if i < 0 || i >= len as i128 {
                    return Err(self.err(
                        RtKind::Index,
                        format!("at({i}) is out of range for a string of size {len}: this would throw out_of_range"),
                    ));
                }
                Ok(Rv::Place(rp.with(Step::Idx(i as usize, ver))))
            }
            "substr" => {
                self.nargs(name, args, 0, 2)?;
                let pos = if args.is_empty() {
                    0
                } else {
                    self.arg_size(&args[0])?
                };
                let cnt = if args.len() > 1 {
                    self.arg_size(&args[1])?
                } else {
                    NPOS
                };
                let s = self.str_ref(&rp)?;
                if pos > s.len() as u64 {
                    return Err(self.pos_err("substr"));
                }
                let end = (pos.saturating_add(cnt)).min(s.len() as u64) as usize;
                let part = s[pos as usize..end].to_vec();
                if is_view {
                    return Ok(Rv::Val(Value::View(Rc::new(part))));
                }
                self.charge(part.len() as u64)?;
                Ok(Rv::Val(Value::string(part)))
            }
            "find" | "rfind" | "find_first_of" | "find_last_of" | "find_first_not_of"
            | "find_last_not_of" => self.str_find(&rp, name, args),
            "compare" => {
                self.nargs(name, args, 1, 5)?;
                let whole = self.str_bytes(&rp)?;
                let (mine, other) = match args.len() {
                    1 => (whole, self.arg_bytes(&args[0])?),
                    3..=5 => {
                        let pos = self.arg_size(&args[0])?;
                        let cnt = self.arg_size(&args[1])?;
                        let o = self.arg_bytes(&args[2])?;
                        let part = self.sub_bytes("compare", &whole, pos, cnt)?;
                        let o = if args.len() >= 4 {
                            let pos2 = self.arg_size(&args[3])?;
                            let cnt2 = if args.len() == 5 {
                                self.arg_size(&args[4])?
                            } else {
                                NPOS
                            };
                            self.sub_bytes("compare", &o, pos2, cnt2)?
                        } else {
                            o
                        };
                        (part, o)
                    }
                    _ => return Err(self.err(RtKind::BadArg, "unsupported compare overload")),
                };
                self.charge(mine.len().min(other.len()) as u64 + 1)?;
                Ok(Rv::Val(Value::Int(match mine.cmp(&other) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                })))
            }
            "append" => {
                self.nargs(name, args, 1, 3)?;
                let add: Vec<u8> = match args.len() {
                    1 => self.arg_bytes(&args[0])?,
                    2 => {
                        // append(n, c), append(first, last) or append(str, pos)
                        let first = self.eval(&args[0])?;
                        if let Some(s) = first.bytes().map(<[u8]>::to_vec) {
                            let pos = self.arg_size(&args[1])?;
                            self.sub_bytes("append", &s, pos, NPOS)?
                        } else {
                            let c = self.eval(&args[1])?;
                            if let (Some(n), Some(c)) = (first.as_i128(), c.as_i128()) {
                                let n = self.count_arg(n)?;
                                self.reserve(n / 8)?;
                                vec![c as u8; n as usize]
                            } else {
                                let vals = self.range_values(&first, &c)?;
                                vals.iter()
                                    .map(|v| v.as_i128().unwrap_or(0) as u8)
                                    .collect()
                            }
                        }
                    }
                    _ => {
                        let s = self.arg_bytes(&args[0])?;
                        let pos = self.arg_size(&args[1])?;
                        let cnt = self.arg_size(&args[2])?;
                        self.sub_bytes("append", &s, pos, cnt)?
                    }
                };
                self.charge(add.len() as u64)?;
                self.str_mut(&rp, true)?.s.extend_from_slice(&add);
                Ok(Rv::Place(rp))
            }
            "assign" => {
                self.nargs(name, args, 1, 2)?;
                let new: Vec<u8> = if args.len() == 1 {
                    self.arg_bytes(&args[0])?
                } else {
                    let n = self.arg_int(&args[0])?;
                    let c = self.arg_int(&args[1])?;
                    let n = self.count_arg(n)?;
                    self.reserve(n / 8)?;
                    vec![c as u8; n as usize]
                };
                self.charge(new.len() as u64)?;
                self.str_mut(&rp, true)?.s = new;
                Ok(Rv::Place(rp))
            }
            "insert" => self.str_insert(&rp, args),
            "erase" => self.str_erase(&rp, args),
            "replace" => {
                self.nargs(name, args, 3, 5)?;
                let first = self.eval(&args[0])?;
                let len = self.str_ref(&rp)?.len();
                let (pos, end, with) = if let Value::Iter(it) = &first {
                    // replace(first, last, str) and replace(first, last, n, c)
                    let (Pos::Idx(i), Value::Iter(j)) = (&it.pos, self.eval(&args[1])?) else {
                        return Err(self.err(RtKind::BadArg, "bad iterator for replace"));
                    };
                    let Pos::Idx(j) = j.pos else {
                        return Err(self.err(RtKind::BadArg, "bad iterator for replace"));
                    };
                    if it.ver != self.str_len_ver(&rp)?.1 {
                        return Err(self.fail(super::invalid_msg()));
                    }
                    if *i > j || j > len {
                        return Err(self.err(RtKind::Index, "replace range is outside the string"));
                    }
                    let with = self.replace_text(args, 2)?;
                    (*i, j, with)
                } else {
                    let pos = self.arg_size(&args[0])?;
                    let cnt = self.arg_size(&args[1])?;
                    if pos > len as u64 {
                        return Err(self.pos_err("replace"));
                    }
                    let end = pos.saturating_add(cnt).min(len as u64) as usize;
                    let with = self.replace_text(args, 2)?;
                    (pos as usize, end, with)
                };
                self.charge((len - pos + with.len()) as u64)?;
                let s = self.str_mut(&rp, true)?;
                s.s.splice(pos..end, with);
                Ok(Rv::Place(rp))
            }
            "resize" => {
                self.nargs(name, args, 1, 2)?;
                let n = self.arg_int(&args[0])?;
                let c = if args.len() > 1 {
                    self.arg_int(&args[1])?
                } else {
                    0
                };
                let n = self.count_arg(n)?;
                self.reserve(n / 8)?;
                self.charge(n)?;
                self.str_mut(&rp, true)?.s.resize(n as usize, c as u8);
                Ok(Rv::Val(Value::Void))
            }
            "reserve" | "shrink_to_fit" => {
                if let Some(a) = args.first() {
                    let n = self.arg_int(a)?;
                    self.reserve(self.count_arg(n)? / 8)?;
                }
                Ok(Rv::Val(Value::Void))
            }
            "swap" => {
                self.nargs(name, args, 1, 1)?;
                let other = self.eval_place(&args[0])?;
                self.swap_places(&rp, &other)?;
                Ok(Rv::Val(Value::Void))
            }
            "remove_prefix" if is_view => {
                let n = self.arg_size(&args[0])?;
                let b = self.str_bytes(&rp)?;
                if n > b.len() as u64 {
                    return Err(self.err(RtKind::Index, "remove_prefix past the end of the view"));
                }
                self.store(&rp, Value::View(Rc::new(b[n as usize..].to_vec())))
                    .map_err(|f| self.fail(f))?;
                Ok(Rv::Val(Value::Void))
            }
            "remove_suffix" if is_view => {
                let n = self.arg_size(&args[0])?;
                let b = self.str_bytes(&rp)?;
                if n > b.len() as u64 {
                    return Err(self.err(RtKind::Index, "remove_suffix past the end of the view"));
                }
                self.store(
                    &rp,
                    Value::View(Rc::new(b[..b.len() - n as usize].to_vec())),
                )
                .map_err(|f| self.fail(f))?;
                Ok(Rv::Val(Value::Void))
            }
            _ => Err(self.err(
                RtKind::BadArg,
                format!("std::string has no member function `{name}` in this shop"),
            )),
        }
    }

    /// The characters of a string-like or `char` value.
    fn val_bytes(&self, v: &Value) -> R<Vec<u8>> {
        match v {
            Value::Char(c) => Ok(vec![*c as u8]),
            v => v.bytes().map(<[u8]>::to_vec).ok_or_else(|| {
                self.err(
                    RtKind::BadArg,
                    "this argument must be a string or a character",
                )
            }),
        }
    }

    /// `s[pos..pos + len]` clamped, or the "pos out of range" error of `what`.
    fn sub_bytes(&self, what: &str, s: &[u8], pos: u64, len: u64) -> R<Vec<u8>> {
        if pos > s.len() as u64 {
            return Err(self.pos_err(what));
        }
        let end = pos.saturating_add(len).min(s.len() as u64) as usize;
        Ok(s[pos as usize..end].to_vec())
    }

    /// The replacement text of `replace` from `args[at..]`: `str`, `str, subpos[, sublen]` or `n, c`.
    fn replace_text(&mut self, args: &'p [Expr], at: usize) -> R<Vec<u8>> {
        let rest = &args[at..];
        let first = self.eval(&rest[0])?;
        match rest.len() {
            1 => self.val_bytes(&first),
            _ if first.bytes().is_some() => {
                let s = self.val_bytes(&first)?;
                let pos = self.arg_size(&rest[1])?;
                let cnt = if rest.len() > 2 {
                    self.arg_size(&rest[2])?
                } else {
                    NPOS
                };
                self.sub_bytes("replace", &s, pos, cnt)
            }
            2 => {
                let n = first.as_i128().unwrap_or(0);
                let c = self.arg_int(&rest[1])?;
                let n = self.count_arg(n)?;
                self.reserve(n / 8)?;
                Ok(vec![c as u8; n as usize])
            }
            _ => Err(self.err(RtKind::BadArg, "unsupported replace overload")),
        }
    }

    pub(super) fn str_len_ver(&self, rp: &Place) -> R<(usize, u32)> {
        match self.get(rp).map_err(|f| self.fail(f))? {
            Value::Str(s) => Ok((s.s.len(), s.ver)),
            Value::View(b) => Ok((b.len(), 0)),
            _ => Err(self.internal("not a string")),
        }
    }

    pub(super) fn swap_places(&mut self, a: &Place, b: &Place) -> R<()> {
        let mut x = self.load(a).map_err(|f| self.fail(f))?;
        let mut y = self.load(b).map_err(|f| self.fail(f))?;
        // containers swap storage: every iterator into either is invalidated
        x.bump();
        y.bump();
        let ox = std::mem::replace(&mut x, Value::Void);
        let oy = std::mem::replace(&mut y, Value::Void);
        // keep version counters moving forward on each place
        let mut nx = oy;
        let mut ny = ox;
        if let (Some(va), Some(vb)) = (self.get_ver(a), self.get_ver(b)) {
            nx.set_ver(va.wrapping_add(1));
            ny.set_ver(vb.wrapping_add(1));
        }
        self.store_raw(a, nx)?;
        self.store_raw(b, ny)?;
        Ok(())
    }

    fn get_ver(&self, p: &Place) -> Option<u32> {
        self.get(p).ok().and_then(Value::ver)
    }

    pub(super) fn store_raw(&mut self, p: &Place, v: Value) -> R<()> {
        match self.loc_mut_r(p)? {
            super::LocM::V(slot) => {
                *slot = v;
                Ok(())
            }
            super::LocM::B(b) => {
                *b = v.as_i128().unwrap_or(0) as u8;
                Ok(())
            }
        }
    }

    fn str_find(&mut self, rp: &Place, name: &str, args: &'p [Expr]) -> R<Rv> {
        self.nargs(name, args, 1, 3)?;
        let needle = self.arg_bytes(&args[0])?;
        let s = self.str_bytes(rp)?;
        let n = s.len();
        let fwd = matches!(name, "find" | "find_first_of" | "find_first_not_of");
        let pos = if args.len() > 1 {
            self.arg_size(&args[1])?
        } else if fwd {
            0
        } else {
            NPOS
        };
        let mut scanned = 1u64;
        let found: Option<usize> = match name {
            "find" => {
                let m = needle.len();
                if pos > n as u64 || m > n {
                    None
                } else {
                    let mut r = None;
                    let mut i = pos as usize;
                    while i + m <= n {
                        scanned += 1;
                        if s[i..i + m] == needle[..] {
                            r = Some(i);
                            break;
                        }
                        i += 1;
                    }
                    scanned += m as u64;
                    r
                }
            }
            "rfind" => {
                let m = needle.len();
                if m > n {
                    None
                } else {
                    let start = (pos.min((n - m) as u64)) as usize;
                    let mut r = None;
                    let mut i = start as isize;
                    while i >= 0 {
                        scanned += 1;
                        let u = i as usize;
                        if s[u..u + m] == needle[..] {
                            r = Some(u);
                            break;
                        }
                        i -= 1;
                    }
                    scanned += m as u64;
                    r
                }
            }
            "find_first_of" | "find_first_not_of" => {
                let want = name == "find_first_of";
                let mut r = None;
                let mut i = pos as usize;
                if pos < n as u64 {
                    while i < n {
                        scanned += 1;
                        if needle.contains(&s[i]) == want {
                            r = Some(i);
                            break;
                        }
                        i += 1;
                    }
                }
                r
            }
            _ => {
                let want = name == "find_last_of";
                let mut r = None;
                if n > 0 {
                    let mut i = pos.min((n - 1) as u64) as isize;
                    while i >= 0 {
                        scanned += 1;
                        if needle.contains(&s[i as usize]) == want {
                            r = Some(i as usize);
                            break;
                        }
                        i -= 1;
                    }
                }
                r
            }
        };
        self.charge(scanned)?;
        Ok(Rv::Val(Value::Size(found.map_or(NPOS, |i| i as u64))))
    }

    fn str_insert(&mut self, rp: &Place, args: &'p [Expr]) -> R<Rv> {
        self.nargs("insert", args, 2, 4)?;
        let first = self.eval(&args[0])?;
        let len = self.str_ref(rp)?.len();
        let (pos, ins): (usize, Vec<u8>) = match &first {
            Value::Iter(it) => {
                let Pos::Idx(i) = it.pos else {
                    return Err(self.err(RtKind::BadArg, "bad iterator for insert"));
                };
                if it.ver != self.str_len_ver(rp)?.1 {
                    return Err(self.fail(super::invalid_msg()));
                }
                let ins = match args.len() {
                    2 => self.arg_bytes(&args[1])?,
                    _ => {
                        let a = self.eval(&args[1])?;
                        let b = self.eval(&args[2])?;
                        if matches!(a, Value::Iter(_)) {
                            let vals = self.range_values(&a, &b)?;
                            vals.iter()
                                .map(|v| v.as_i128().unwrap_or(0) as u8)
                                .collect()
                        } else {
                            let n = a.as_i128().unwrap_or(0);
                            let c = b.as_i128().unwrap_or(0);
                            let n = self.count_arg(n)?;
                            vec![c as u8; n as usize]
                        }
                    }
                };
                (i, ins)
            }
            v => {
                let pos = v.as_i128().unwrap_or(-1) as u64;
                if pos > len as u64 {
                    return Err(self.pos_err("insert"));
                }
                let ins = match args.len() {
                    2 => self.arg_bytes(&args[1])?,
                    _ => {
                        let a = self.eval(&args[1])?;
                        if a.bytes().is_some() {
                            // insert(pos, str, subpos[, sublen])
                            let s = self.val_bytes(&a)?;
                            let sub = self.arg_size(&args[2])?;
                            let cnt = if args.len() > 3 {
                                self.arg_size(&args[3])?
                            } else {
                                NPOS
                            };
                            self.sub_bytes("insert", &s, sub, cnt)?
                        } else if args.len() == 3 {
                            // insert(pos, n, c)
                            let n = a.as_i128().unwrap_or(0);
                            let c = self.arg_int(&args[2])?;
                            let n = self.count_arg(n)?;
                            vec![c as u8; n as usize]
                        } else {
                            return Err(self.err(RtKind::BadArg, "unsupported insert overload"));
                        }
                    }
                };
                (pos as usize, ins)
            }
        };
        self.reserve(ins.len() as u64 / 8)?;
        self.charge((len - pos + ins.len()) as u64)?;
        let s = self.str_mut(rp, true)?;
        s.s.splice(pos..pos, ins);
        let ver = s.ver;
        if matches!(first, Value::Iter(_)) {
            let mut it = self.make_iter(rp, false, false)?;
            it.pos = Pos::Idx(pos);
            it.ver = ver;
            return Ok(Rv::Val(Value::Iter(Box::new(it))));
        }
        Ok(Rv::Place(rp.clone()))
    }

    fn str_erase(&mut self, rp: &Place, args: &'p [Expr]) -> R<Rv> {
        self.nargs("erase", args, 0, 2)?;
        let len = self.str_ref(rp)?.len();
        if args.is_empty() {
            self.charge(len as u64)?;
            self.str_mut(rp, true)?.s.clear();
            return Ok(Rv::Place(rp.clone()));
        }
        let first = self.eval(&args[0])?;
        if let Value::Iter(it) = &first {
            let Pos::Idx(i) = it.pos else {
                return Err(self.err(RtKind::BadArg, "bad iterator for erase"));
            };
            if it.ver != self.str_len_ver(rp)?.1 {
                return Err(self.fail(super::invalid_msg()));
            }
            let j = if args.len() == 2 {
                match self.eval(&args[1])? {
                    Value::Iter(b) => match b.pos {
                        Pos::Idx(j) => j,
                        _ => return Err(self.err(RtKind::BadArg, "bad iterator for erase")),
                    },
                    _ => return Err(self.err(RtKind::BadArg, "bad iterator for erase")),
                }
            } else {
                i + 1
            };
            if i > j || j > len {
                return Err(self.err(RtKind::Index, "erase range is outside the string"));
            }
            self.charge((len - i) as u64)?;
            let s = self.str_mut(rp, true)?;
            s.s.drain(i..j);
            let ver = s.ver;
            let mut ni = self.make_iter(rp, false, false)?;
            ni.pos = Pos::Idx(i);
            ni.ver = ver;
            return Ok(Rv::Val(Value::Iter(Box::new(ni))));
        }
        let pos = first.as_i128().unwrap_or(-1) as u64;
        let cnt = if args.len() > 1 {
            self.arg_size(&args[1])?
        } else {
            NPOS
        };
        if pos > len as u64 {
            return Err(self.pos_err("erase"));
        }
        let end = pos.saturating_add(cnt).min(len as u64) as usize;
        self.charge((len - pos as usize) as u64)?;
        self.str_mut(rp, true)?.s.drain(pos as usize..end);
        Ok(Rv::Place(rp.clone()))
    }

    // ----- optional -----

    fn m_opt(&mut self, rp: Place, name: &'p str, args: &'p [Expr]) -> R<Rv> {
        let has = match self.get(&rp).map_err(|f| self.fail(f))? {
            Value::Opt(o) => o.v.is_some(),
            _ => false,
        };
        match name {
            "has_value" => Ok(Rv::Val(Value::Bool(has))),
            "value" => {
                if !has {
                    return Err(self.err(
                        RtKind::Empty,
                        "value() on an empty std::optional: this would throw bad_optional_access",
                    ));
                }
                Ok(Rv::Place(rp.with(Step::Opt)))
            }
            "value_or" => {
                self.nargs(name, args, 1, 1)?;
                if has {
                    // the alternative is still evaluated in C++ (function argument)
                    let ety = match self.get(&rp).map_err(|f| self.fail(f))? {
                        Value::Opt(o) => (*o.ety).clone(),
                        _ => Ty::Unknown,
                    };
                    let _ = self.eval_conv(&args[0], &ety)?;
                    let v = self.materialize(&rp.with(Step::Opt))?;
                    return Ok(Rv::Val(v));
                }
                let ety = match self.get(&rp).map_err(|f| self.fail(f))? {
                    Value::Opt(o) => (*o.ety).clone(),
                    _ => Ty::Unknown,
                };
                Ok(Rv::Val(self.eval_conv(&args[0], &ety)?))
            }
            "reset" => {
                if let Value::Opt(o) = self.get_mut(&rp)? {
                    o.v = None;
                }
                Ok(Rv::Val(Value::Void))
            }
            "emplace" => {
                let ety = match self.get(&rp).map_err(|f| self.fail(f))? {
                    Value::Opt(o) => (*o.ety).clone(),
                    _ => Ty::Unknown,
                };
                let v = self.construct_ty(&ety, args, false)?;
                if let Value::Opt(o) = self.get_mut(&rp)? {
                    o.v = Some(v);
                }
                Ok(Rv::Place(rp.with(Step::Opt)))
            }
            _ => Err(self.err(
                RtKind::BadArg,
                format!("std::optional has no member function `{name}`"),
            )),
        }
    }
}
