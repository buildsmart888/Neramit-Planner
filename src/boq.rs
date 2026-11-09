//! Import BOQ: a bill of quantities in Excel (.xlsx) or CSV becomes the plan's task outline —
//! one task per item with its amount as the task's fixed cost (so the S-Curve and the payment
//! schedule weigh it), groups as summary tasks.
//!
//! Thai BOQs (แบบ ปร.4 and contractors' own sheets) have no fixed layout, so the header row is
//! found from the words in it (ลำดับ / รายการ / จำนวน / หน่วย / ค่าวัสดุ / ค่าแรง / รวม ...),
//! including two-row headers whose top cells are merged across "ราคาต่อหน่วย | จำนวนเงิน".
//! The detected columns can be changed in the import dialog before anything is added.
//! Outline levels come from the item numbers (1, 1.1, 1.1.1 ...); without numbers, rows with no
//! quantity and no amount are groups. Subtotal, VAT and Factor F rows are left out.

use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Empty,
    Num(f64),
    Text(String),
}

impl Val {
    pub fn text(&self) -> String {
        match self {
            Val::Empty => String::new(),
            Val::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            Val::Text(s) => s.trim().to_string(),
        }
    }

    /// The cell as a number: numbers, and text such as "1,250.50" or "฿ 300"; "-" is zero.
    pub fn num(&self) -> Option<f64> {
        match self {
            Val::Num(n) => Some(*n),
            Val::Text(s) => {
                let t: String = s.trim().chars().filter(|c| !matches!(c, ',' | ' ' | '฿' | '$')).collect();
                if t == "-" {
                    return Some(0.0);
                }
                t.parse::<f64>().ok()
            }
            Val::Empty => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<Val>>,
}

// ======================================================================== reading files

pub fn read_file(path: &Path) -> Result<Vec<Sheet>, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read the file: {e}"))?;
    match ext.as_str() {
        "xlsx" | "xlsm" => read_xlsx(&bytes),
        "csv" | "txt" => Ok(vec![Sheet { name: "CSV".into(), rows: read_csv(&bytes) }]),
        "xls" => Err("Old Excel files (.xls) cannot be read: open the file in Excel and save it as .xlsx.".into()),
        _ => Err("Choose an Excel workbook (.xlsx) or a CSV file.".into()),
    }
}

/// Files in a ZIP archive (stored or deflated): name → data.
fn unzip(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let u16le = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let u32le = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let bad = || "The workbook is damaged or not an .xlsx file.".to_string();
    // end of central directory
    let from = data.len().saturating_sub(65_557);
    let eocd = (from..data.len().saturating_sub(21)).rev().find(|&i| u32le(i) == Some(0x0605_4b50)).ok_or_else(bad)?;
    let count = u16le(eocd + 10).ok_or_else(bad)?;
    let mut at = u32le(eocd + 16).ok_or_else(bad)?;
    let mut out = Vec::new();
    for _ in 0..count {
        if u32le(at) != Some(0x0201_4b50) {
            return Err(bad());
        }
        let method = u16le(at + 10).ok_or_else(bad)?;
        let csize = u32le(at + 20).ok_or_else(bad)?;
        let nlen = u16le(at + 28).ok_or_else(bad)?;
        let xlen = u16le(at + 30).ok_or_else(bad)?;
        let clen = u16le(at + 32).ok_or_else(bad)?;
        let local = u32le(at + 42).ok_or_else(bad)?;
        let name = String::from_utf8_lossy(data.get(at + 46..at + 46 + nlen).ok_or_else(bad)?).to_string();
        at += 46 + nlen + xlen + clen;
        if u32le(local) != Some(0x0403_4b50) {
            return Err(bad());
        }
        let start = local + 30 + u16le(local + 26).ok_or_else(bad)? + u16le(local + 28).ok_or_else(bad)?;
        let raw = data.get(start..start + csize).ok_or_else(bad)?;
        let body = match method {
            0 => raw.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec(raw).map_err(|_| bad())?,
            _ => continue,
        };
        out.push((name, body));
    }
    Ok(out)
}

fn attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == key || a.key.local_name().as_ref() == key).map(|a| String::from_utf8_lossy(&a.value).to_string())
}

fn unescape(s: &str) -> String {
    quick_xml::escape::unescape(s).map(|c| c.to_string()).unwrap_or_else(|_| s.to_string())
}

/// "AB12" → (row 11, column 27), zero-based.
fn cell_ref(r: &str) -> Option<(usize, usize)> {
    let letters: String = r.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = &r[letters.len()..];
    let col = letters.bytes().fold(0usize, |a, b| a * 26 + (b.to_ascii_uppercase() - b'A' + 1) as usize);
    Some((digits.parse::<usize>().ok()?.checked_sub(1)?, col.checked_sub(1)?))
}

pub fn read_xlsx(bytes: &[u8]) -> Result<Vec<Sheet>, String> {
    use quick_xml::events::Event;
    let files = unzip(bytes)?;
    let get = |n: &str| files.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|(_, v)| v.as_slice());
    // shared strings
    let mut shared: Vec<String> = Vec::new();
    if let Some(x) = get("xl/sharedStrings.xml") {
        let mut r = quick_xml::Reader::from_reader(x);
        let (mut cur, mut in_t, mut in_ph) = (String::new(), false, false);
        let mut buf = Vec::new();
        loop {
            match r.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => match e.local_name().as_ref() {
                    b"si" => cur.clear(),
                    b"t" => in_t = true,
                    b"rPh" => in_ph = true,
                    _ => {}
                },
                Ok(Event::Empty(e)) if e.local_name().as_ref() == b"si" => shared.push(String::new()),
                Ok(Event::Text(t)) if in_t && !in_ph => cur.push_str(&unescape(&String::from_utf8_lossy(&t))),
                Ok(Event::End(e)) => match e.local_name().as_ref() {
                    b"si" => shared.push(std::mem::take(&mut cur)),
                    b"t" => in_t = false,
                    b"rPh" => in_ph = false,
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
    }
    // sheets: names in workbook.xml, files through workbook.xml.rels
    let mut rels: Vec<(String, String)> = Vec::new();
    if let Some(x) = get("xl/_rels/workbook.xml.rels") {
        let mut r = quick_xml::Reader::from_reader(x);
        let mut buf = Vec::new();
        loop {
            match r.read_event_into(&mut buf) {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.local_name().as_ref() == b"Relationship" => {
                    if let (Some(id), Some(t)) = (attr(&e, b"Id"), attr(&e, b"Target")) {
                        let t = t.trim_start_matches('/').to_string();
                        rels.push((id, if t.starts_with("xl/") { t } else { format!("xl/{t}") }));
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
    }
    let mut sheets: Vec<(String, String)> = Vec::new();
    if let Some(x) = get("xl/workbook.xml") {
        let mut r = quick_xml::Reader::from_reader(x);
        let mut buf = Vec::new();
        loop {
            match r.read_event_into(&mut buf) {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.local_name().as_ref() == b"sheet" => {
                    let name = attr(&e, b"name").map(|n| unescape(&n)).unwrap_or_default();
                    if let Some(id) = attr(&e, b"id") {
                        if let Some((_, t)) = rels.iter().find(|(k, _)| *k == id) {
                            sheets.push((name, t.clone()));
                        }
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
    }
    if sheets.is_empty() {
        sheets = files.iter().filter(|(k, _)| k.starts_with("xl/worksheets/") && k.ends_with(".xml")).map(|(k, _)| (k.trim_start_matches("xl/worksheets/").trim_end_matches(".xml").to_string(), k.clone())).collect();
    }
    let mut out = Vec::new();
    for (name, file) in sheets {
        let Some(x) = get(&file) else { continue };
        let mut rows: Vec<Vec<Val>> = Vec::new();
        let mut r = quick_xml::Reader::from_reader(x);
        let mut buf = Vec::new();
        let (mut pos, mut kind, mut val, mut in_v, mut in_is_t) = ((0usize, 0usize), String::new(), String::new(), false, false);
        let mut next_col = 0usize;
        let mut row_no = 0usize;
        loop {
            match r.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.local_name().as_ref() == b"row" => {
                    row_no = attr(&e, b"r").and_then(|v| v.parse::<usize>().ok()).map(|v| v - 1).unwrap_or(row_no);
                    next_col = 0;
                }
                Ok(Event::Start(e)) if e.local_name().as_ref() == b"c" => {
                    pos = attr(&e, b"r").and_then(|v| cell_ref(&v)).unwrap_or((row_no, next_col));
                    kind = attr(&e, b"t").unwrap_or_default();
                    val.clear();
                }
                Ok(Event::Start(e)) if e.local_name().as_ref() == b"v" => in_v = true,
                Ok(Event::Start(e)) if e.local_name().as_ref() == b"t" => in_is_t = true,
                Ok(Event::Text(t)) if in_v || in_is_t => val.push_str(&unescape(&String::from_utf8_lossy(&t))),
                Ok(Event::End(e)) => match e.local_name().as_ref() {
                    b"v" => in_v = false,
                    b"t" => in_is_t = false,
                    b"c" => {
                        let v = match kind.as_str() {
                            "s" => val.trim().parse::<usize>().ok().and_then(|k| shared.get(k)).map(|s| Val::Text(s.clone())).unwrap_or(Val::Empty),
                            "str" | "inlineStr" => Val::Text(val.clone()),
                            "b" => Val::Text(if val.trim() == "1" { "TRUE".into() } else { "FALSE".into() }),
                            "e" => Val::Empty,
                            _ => val.trim().parse::<f64>().map(Val::Num).unwrap_or(if val.trim().is_empty() { Val::Empty } else { Val::Text(val.clone()) }),
                        };
                        let (rr, cc) = pos;
                        if rr < 20_000 && cc < 200 {
                            if rows.len() <= rr {
                                rows.resize(rr + 1, Vec::new());
                            }
                            if rows[rr].len() <= cc {
                                rows[rr].resize(cc + 1, Val::Empty);
                            }
                            rows[rr][cc] = v;
                        }
                        next_col = cc + 1;
                    }
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
        out.push(Sheet { name, rows });
    }
    if out.is_empty() {
        return Err("The workbook has no sheets.".into());
    }
    Ok(out)
}

/// CSV in UTF-8 (with or without BOM) or Thai Windows-874; comma, semicolon or tab separated.
pub fn read_csv(bytes: &[u8]) -> Vec<Vec<Val>> {
    let text = match std::str::from_utf8(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| if (0xA1..=0xFB).contains(&b) { char::from_u32(0x0E00 + (b as u32 - 0xA0)).unwrap_or('?') } else { b as char }).collect(),
    };
    let first = text.lines().next().unwrap_or("");
    let sep = [',', ';', '\t'].into_iter().max_by_key(|c| first.matches(*c).count()).unwrap_or(',');
    let mut rows = Vec::new();
    let mut row: Vec<Val> = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let push = |cell: &mut String, row: &mut Vec<Val>| {
        let s = std::mem::take(cell);
        row.push(if s.trim().is_empty() { Val::Empty } else { Val::Text(s) });
    };
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => push(&mut cell, &mut row),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                push(&mut cell, &mut row);
                rows.push(std::mem::take(&mut row));
            }
            c => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        push(&mut cell, &mut row);
        rows.push(row);
    }
    rows
}

// ======================================================================== finding the columns

/// Which column holds what (zero-based); `None` = not in this sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Columns {
    pub header_row: usize,
    /// rows the header takes (1 or 2)
    pub header_rows: usize,
    pub no: Option<usize>,
    pub desc: Option<usize>,
    pub qty: Option<usize>,
    pub unit: Option<usize>,
    /// total amount of the item (material + labour)
    pub amount: Option<usize>,
    /// used when there is no total column: material amount + labour amount
    pub material: Option<usize>,
    pub labour: Option<usize>,
    /// used when there are no amounts: quantity × unit price
    pub unit_price: Option<usize>,
}

fn norm(s: &str) -> String {
    s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The header text of every column: the header row, joined with the row under it for two-row
/// headers (a merged top cell repeats over the columns under it).
pub fn header_labels(rows: &[Vec<Val>], hr: usize, two: bool) -> Vec<String> {
    let width = rows.iter().skip(hr).take(2).map(|r| r.len()).max().unwrap_or(0);
    let top: Vec<String> = (0..width).map(|c| rows.get(hr).and_then(|r| r.get(c)).map(|v| v.text()).unwrap_or_default()).collect();
    if !two {
        return top.iter().map(|s| norm(s)).collect();
    }
    let bottom: Vec<String> = (0..width).map(|c| rows.get(hr + 1).and_then(|r| r.get(c)).map(|v| v.text()).unwrap_or_default()).collect();
    let mut filled = top.clone();
    for c in 1..width {
        if filled[c].is_empty() && !bottom[c].is_empty() && !bottom[c - 1].is_empty() {
            filled[c] = filled[c - 1].clone();
        }
    }
    (0..width).map(|c| norm(&format!("{} {}", filled[c], bottom[c]))).collect()
}

fn has(s: &str, words: &[&str]) -> bool {
    words.iter().any(|w| s.contains(w))
}

fn is_desc(h: &str) -> bool {
    has(h, &["รายการ", "รายละเอียด", "description", "ลักษณะงาน"]) || h == "งาน" || h == "item description"
}
fn is_qty(h: &str) -> bool {
    (has(h, &["จำนวน", "ปริมาณ", "qty", "quantity"]) && !has(h, &["เงิน", "amount"])) || h == "q'ty"
}
fn is_unit(h: &str) -> bool {
    (has(h, &["หน่วย", "unit"]) && !has(h, &["ราคา", "ต่อหน่วย", "price", "rate", "cost"])) || h == "uom"
}
fn is_money(h: &str) -> bool {
    has(h, &["จำนวนเงิน", "เป็นเงิน", "amount", "ราคารวม", "total", "รวม"])
}
fn is_no(h: &str) -> bool {
    matches!(h, "ที่" | "ลำดับ" | "ลำดับที่" | "no" | "no." | "item" | "รหัส" | "code" | "ข้อ") || h.starts_with("ลำดับ")
}

/// Find the header row (in the first 40 rows) and the columns.
pub fn detect(rows: &[Vec<Val>]) -> Option<Columns> {
    let mut best: Option<(i32, usize)> = None;
    for hr in 0..rows.len().min(40) {
        let labels = header_labels(rows, hr, false);
        if !labels.iter().any(|h| is_desc(h)) {
            continue;
        }
        let score = labels.iter().filter(|h| is_desc(h) || is_qty(h) || is_unit(h) || is_money(h) || is_no(h)).count() as i32;
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, hr));
        }
    }
    let (_, hr) = best?;
    // a second header row: text under the header and no numbers in it
    let next = rows.get(hr + 1);
    let two = next.is_some_and(|r| {
        let texts = r.iter().filter(|v| matches!(v, Val::Text(t) if !t.trim().is_empty() && t.trim().parse::<f64>().is_err())).count();
        let nums = r.iter().filter(|v| matches!(v, Val::Num(_))).count();
        texts >= 2 && nums == 0 && r.iter().map(|v| v.text()).any(|t| has(&norm(&t), &["ราคา", "จำนวนเงิน", "เงิน", "หน่วยละ", "price", "amount", "rate", "unit"]))
    });
    let labels = header_labels(rows, hr, two);
    let find = |f: &dyn Fn(&str) -> bool| labels.iter().position(|h| f(h));
    let rfind = |f: &dyn Fn(&str) -> bool| labels.iter().rposition(|h| f(h));
    let mut c = Columns { header_row: hr, header_rows: if two { 2 } else { 1 }, ..Default::default() };
    c.desc = find(&is_desc);
    c.no = find(&is_no).filter(|&k| Some(k) != c.desc);
    c.qty = find(&is_qty);
    c.unit = find(&is_unit).filter(|&k| Some(k) != c.qty);
    let money_amount = |h: &str| has(h, &["จำนวนเงิน", "เป็นเงิน", "amount", "ราคารวม", "รวม"]) && !has(h, &["ต่อหน่วย", "หน่วยละ", "unit price", "rate"]);
    // material and labour amounts (not their unit prices): "ค่าวัสดุ จำนวนเงิน", "ค่าแรง (บาท)"
    let rate = |h: &str| has(h, &["ต่อหน่วย", "หน่วยละ", "unit price", "rate", "/หน่วย"]);
    c.material = find(&|h: &str| has(h, &["วัสดุ", "material"]) && !has(h, &["แรง", "labour", "labor", "รวม"]) && !rate(h));
    c.labour = find(&|h: &str| has(h, &["แรง", "labour", "labor"]) && !has(h, &["วัสดุ", "material", "รวม"]) && !rate(h));
    // the total: a "รวม ..." / total column right of the others, else any amount column
    c.amount = rfind(&|h: &str| (has(h, &["รวม", "total"]) && !has(h, &["ต่อหน่วย", "หน่วยละ", "unit price"])) || has(h, &["ค่าวัสดุและค่าแรง", "รวมเป็นเงิน", "ราคารวม"]))
        .or_else(|| if c.material.is_none() && c.labour.is_none() { rfind(&money_amount) } else { None });
    if c.amount.is_some() && (c.amount == c.material || c.amount == c.labour) {
        c.amount = None;
    }
    c.unit_price = find(&|h: &str| has(h, &["ราคาต่อหน่วย", "หน่วยละ", "unit price", "rate"]) && !has(h, &["วัสดุ", "แรง", "material", "labour"]));
    // no numbering column named: the column left of the description, when it holds item numbers
    if c.no.is_none() {
        if let Some(d) = c.desc.filter(|&d| d > 0) {
            let body = rows.iter().skip(hr + c.header_rows).take(30);
            let numbered = body.filter(|r| r.get(d - 1).is_some_and(|v| numbering(&v.text()).is_some())).count();
            if numbered >= 2 {
                c.no = Some(d - 1);
            }
        }
    }
    c.desc.map(|_| c)
}

// ======================================================================== items

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub no: String,
    pub name: String,
    pub level: u8,
    pub qty: Option<f64>,
    pub unit: String,
    pub amount: f64,
    /// a group (summary task): no quantity and no amount of its own
    pub group: bool,
    /// lines under the item that only describe it ("- ขนาด 20x20 ซม.")
    pub notes: String,
}

/// Outline depth of an item number: "1" → 1, "1.2" → 2, "2.1.3." → 3, "ก" / "A" / "I" → 1.
fn numbering(s: &str) -> Option<u8> {
    let s = s.trim().trim_end_matches(['.', ')']);
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
        return Some(parts.len().min(9) as u8);
    }
    // Thai letters (ก, ข), Latin letters or Roman numerals as group marks
    let one = s.chars().count() <= 4 && (s.chars().all(|c| ('\u{0E01}'..='\u{0E2E}').contains(&c)) || s.chars().all(|c| c.is_ascii_alphabetic()));
    if one {
        return Some(1);
    }
    // "ก.1", "A.2"
    if parts.len() >= 2 && parts[1..].iter().all(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty()) && parts[0].chars().count() <= 2 {
        return Some(parts.len() as u8);
    }
    None
}

/// Subtotals, VAT, Factor F and similar rows that are not work items.
fn is_summary_line(name: &str) -> bool {
    let n = norm(name);
    let starts = ["รวม", "ยอดรวม", "สรุป", "ภาษีมูลค่าเพิ่ม", "ภาษี", "vat", "total", "sub total", "subtotal", "grand total", "ค่า factor", "factor f", "ค่าดำเนินการ", "กำไร", "ส่วนลด", "หมายเหตุ", "(รวม"];
    starts.iter().any(|s| n.starts_with(s)) || n == "-" || n.contains("ยกไป") || n.contains("ยกมา")
}

pub fn items(rows: &[Vec<Val>], c: &Columns) -> Vec<Item> {
    let Some(dc) = c.desc else { return Vec::new() };
    let cell = |r: &Vec<Val>, k: Option<usize>| k.and_then(|k| r.get(k)).cloned().unwrap_or(Val::Empty);
    let mut out: Vec<Item> = Vec::new();
    let mut numbered = 0;
    for r in rows.iter().skip(c.header_row + c.header_rows) {
        let name = cell(r, Some(dc)).text();
        let no = cell(r, c.no).text();
        if name.is_empty() || is_summary_line(&name) {
            continue;
        }
        let qty = cell(r, c.qty).num().filter(|q| *q != 0.0);
        let unit = cell(r, c.unit).text();
        let amount = match c.amount {
            Some(_) => cell(r, c.amount).num(),
            None => match (cell(r, c.material).num(), cell(r, c.labour).num()) {
                (None, None) => None,
                (m, l) => Some(m.unwrap_or(0.0) + l.unwrap_or(0.0)),
            },
        }
        .or_else(|| Some(qty? * cell(r, c.unit_price).num()?))
        .unwrap_or(0.0);
        let level = numbering(&no);
        if level.is_some() {
            numbered += 1;
        }
        let blank = qty.is_none() && amount.abs() < 1e-9 && unit.is_empty();
        // a description line of the item above
        if blank && level.is_none() && (name.starts_with('-') || name.starts_with('*') || name.starts_with('•')) {
            if let Some(prev) = out.last_mut() {
                if !prev.notes.is_empty() {
                    prev.notes.push('\n');
                }
                prev.notes.push_str(&name);
                continue;
            }
        }
        out.push(Item { no, name, level: level.unwrap_or(0), qty, unit, amount, group: blank, notes: String::new() });
    }
    // levels: from the numbers when most items have one, else groups above their items
    let use_numbers = numbered * 2 >= out.len().max(1);
    let mut last_group: u8 = 0;
    let mut prev: u8 = 0;
    for it in out.iter_mut() {
        let lvl = if use_numbers && it.level > 0 {
            it.level
        } else if it.group {
            if use_numbers { (prev.max(1)).min(last_group + 1).max(1) } else { 1 }
        } else if last_group > 0 {
            last_group + 1
        } else {
            1
        };
        let lvl = lvl.min(prev + 1).max(1);
        if it.group {
            last_group = lvl;
        }
        it.level = lvl;
        prev = lvl;
    }
    out
}

// ======================================================================== into the plan

/// Add the items to the end of the plan as tasks (groups → summary tasks). `days`: duration of
/// each item; `chain`: link the items one after another (finish-to-start).
pub fn add_to_project(p: &mut crate::model::Project, list: &[Item], days: i32, chain: bool) -> Result<usize, String> {
    // Validate before mutating either the schedule or native controls.
    for it in list {
        if !it.amount.is_finite() || it.amount < 0.0 ||
           it.qty.is_some_and(|q| !q.is_finite() || q < 0.0) {
            return Err(format!("Invalid BOQ quantity/amount for {}", it.name));
        }
    }
    let mpd = p.calendar.minutes_per_day();
    let mut prev_leaf: Option<u32> = None;
    let mut n = 0;
    for (k, it) in list.iter().enumerate() {
        let mut t = p.new_task(&it.name);
        t.outline_level = it.level;
        let has_children = list.get(k + 1).is_some_and(|nx| nx.level > it.level);
        let mut notes = Vec::new();
        if !it.no.is_empty() {
            notes.push(format!("BOQ {}", it.no));
        }
        if let Some(q) = it.qty {
            let q = if q.fract() == 0.0 { format!("{}", q as i64) } else { format!("{q}") };
            notes.push(format!("{} {q} {}", crate::i18n::tr("Quantity"), it.unit).trim().to_string());
        }
        if !it.notes.is_empty() {
            notes.push(it.notes.clone());
        }
        t.notes = notes.join("\n");
        if !has_children {
            t.duration = days.max(0);
            t.dur_min = days.max(0) as i64 * mpd;
            t.milestone = days == 0;
            t.fixed_cost = it.amount;
            if chain {
                if let Some(u) = prev_leaf {
                    t.predecessors.push(crate::model::Link { pred_uid: u, ..Default::default() });
                }
            }
            prev_leaf = Some(t.uid);
        }
        let uid = t.uid;
        p.tasks.push(t);
        if !has_children {
            crate::controls::add_boq_line(p, uid, it.no.clone(), it.name.clone(),
                                          it.qty, it.unit.clone(), it.amount)?;
        }
        n += 1;
    }
    Ok(n)
}

// ======================================================================== import dialog

#[derive(Clone, Debug)]
pub struct BoqState {
    pub file: String,
    pub sheets: Vec<Sheet>,
    pub sheet: usize,
    pub cols: Columns,
    /// duration of each item, working days
    pub days: i32,
    pub chain: bool,
    pub error: Option<String>,
}

/// Project > Import BOQ: pick the file, find the sheet with the most items, open the dialog.
pub fn open(app: &mut crate::app::App, path: &Path) {
    match read_file(path) {
        Ok(sheets) => {
            let best = (0..sheets.len()).max_by_key(|&k| detect(&sheets[k].rows).map(|c| items(&sheets[k].rows, &c).len()).unwrap_or(0)).unwrap_or(0);
            let cols = detect(&sheets[best].rows).unwrap_or_default();
            let error = (cols.desc.is_none()).then(|| crate::i18n::tr("No BOQ header was found: choose the columns below.").to_string());
            app.dialog = Some(crate::dialogs::Dialog::Boq(BoqState {
                file: path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(),
                sheets,
                sheet: best,
                cols,
                days: 1,
                chain: false,
                error,
            }));
        }
        Err(e) => app.show_error(e),
    }
}

fn col_name(k: usize) -> String {
    let mut s = String::new();
    let mut n = k + 1;
    while n > 0 {
        s.insert(0, (b'A' + ((n - 1) % 26) as u8) as char);
        n = (n - 1) / 26;
    }
    s
}

/// The dialog; `Some(true)` imports, `Some(false)` cancels.
pub fn dialog(app: &mut crate::app::App, ctx: &eframe::egui::Context, st: &mut BoqState) -> Option<bool> {
    use crate::i18n::{tr, trf};
    use crate::theme;
    use eframe::egui::{self, RichText};
    let mut done = None;
    crate::theme::modal(egui::Id::unique("boq_import")).show(ctx, |ui| {
        crate::theme::dialog_spacing(ui);
        ui.set_width(760.0);
        ui.label(RichText::new(tr("Import BOQ")).strong().size(14.0));
        ui.label(RichText::new(crate::thai::shape(&st.file)).color(theme::TEXT_DIM));
        ui.add_space(6.0);
        if st.sheets.len() > 1 {
            let before = st.sheet;
            ui.horizontal(|ui| {
                ui.label(tr("Sheet:"));
                egui::ComboBox::from_id_salt("boq_sheet").width(220.0).selected_text(crate::thai::shape(&st.sheets[st.sheet].name).to_string()).show_ui(ui, |ui| {
                    for k in 0..st.sheets.len() {
                        ui.selectable_value(&mut st.sheet, k, crate::thai::shape(&st.sheets[k].name).to_string());
                    }
                });
            });
            if st.sheet != before {
                st.cols = detect(&st.sheets[st.sheet].rows).unwrap_or_default();
            }
        }
        let rows = &st.sheets[st.sheet].rows;
        // header row and the columns
        let mut hr = st.cols.header_row as i32 + 1;
        let mut two = st.cols.header_rows == 2;
        ui.horizontal(|ui| {
            ui.label(tr("Header row:"));
            ui.add(egui::DragValue::new(&mut hr).range(1..=rows.len().max(1) as i32));
            ui.checkbox(&mut two, tr("Two-row header"));
        });
        if hr as usize - 1 != st.cols.header_row || two != (st.cols.header_rows == 2) {
            st.cols.header_row = hr as usize - 1;
            st.cols.header_rows = if two { 2 } else { 1 };
        }
        let labels = header_labels(rows, st.cols.header_row, st.cols.header_rows == 2);
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(labels.len());
        let opt = |k: Option<usize>| match k {
            Some(k) => format!("{}: {}", col_name(k), labels.get(k).map(|s| s.as_str()).unwrap_or("")),
            None => tr("(none)").to_string(),
        };
        let pick = |ui: &mut egui::Ui, id: &str, label: &str, v: &mut Option<usize>| {
            ui.label(tr(label));
            egui::ComboBox::from_id_salt(id).width(230.0).selected_text(crate::thai::shape(&opt(*v)).to_string()).show_ui(ui, |ui| {
                ui.selectable_value(v, None, tr("(none)"));
                for k in 0..width {
                    ui.selectable_value(v, Some(k), crate::thai::shape(&opt(Some(k))).to_string());
                }
            });
        };
        let mut c = st.cols;
        egui::Grid::new("boq_cols").num_columns(4).spacing([10.0, 6.0]).show(ui, |ui| {
            pick(ui, "boq_no", "Item no.:", &mut c.no);
            pick(ui, "boq_desc", "Description:", &mut c.desc);
            ui.end_row();
            pick(ui, "boq_qty", "Quantity:", &mut c.qty);
            pick(ui, "boq_unit", "Unit:", &mut c.unit);
            ui.end_row();
            pick(ui, "boq_amt", "Total amount:", &mut c.amount);
            pick(ui, "boq_price", "Unit price:", &mut c.unit_price);
            ui.end_row();
            pick(ui, "boq_mat", "Material amount:", &mut c.material);
            pick(ui, "boq_lab", "Labour amount:", &mut c.labour);
            ui.end_row();
        });
        st.cols = c;
        ui.label(RichText::new(tr("Without a total column the amount is material + labour, or quantity × unit price.")).color(theme::TEXT_DIM).size(11.5));
        // preview
        let list = items(rows, &st.cols);
        let total: f64 = list.iter().map(|i| i.amount).sum();
        let groups = list.iter().filter(|i| i.group).count();
        ui.add_space(6.0);
        ui.label(RichText::new(trf("{} items, {} groups, total {}", &[&(list.len() - groups).to_string(), &groups.to_string(), &crate::util::fmt_money(total)])).strong());
        if crate::i18n::is_thai() && total > 0.0 {
            ui.label(RichText::new(crate::thai::shape(&format!("({})", crate::thai::baht_text(total)))).color(theme::TEXT_DIM));
        }
        egui::Frame::new().stroke(egui::Stroke::new(1.0, theme::GRID_LINE)).inner_margin(4.0).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
                egui::Grid::new("boq_prev").striped(true).num_columns(4).spacing([12.0, 3.0]).min_col_width(40.0).show(ui, |ui| {
                    for h in ["Item no.", "Item description", "Quantity", "Amount"] {
                        ui.label(RichText::new(tr(h)).strong().color(theme::GREEN));
                    }
                    ui.end_row();
                    for it in list.iter().take(300) {
                        ui.label(&it.no);
                        let name = format!("{}{}", "    ".repeat(it.level.saturating_sub(1) as usize), it.name);
                        let mut rt = RichText::new(crate::thai::shape(&name).to_string());
                        if it.group {
                            rt = rt.strong();
                        }
                        ui.allocate_ui_with_layout(egui::vec2(400.0, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_min_width(400.0);
                            ui.add(egui::Label::new(rt).truncate());
                        });
                        ui.label(crate::thai::shape(&it.qty.map(|q| format!("{} {}", crate::util::fmt_num(q), it.unit)).unwrap_or_default()).to_string());
                        ui.label(if it.amount != 0.0 { crate::util::fmt_money(it.amount) } else { String::new() });
                        ui.end_row();
                    }
                });
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(tr("Duration of each item:"));
            ui.add(egui::DragValue::new(&mut st.days).range(0..=365).suffix(format!(" {}", tr("days"))));
            ui.checkbox(&mut st.chain, tr("Link the items one after another"));
        });
        ui.label(RichText::new(tr("Items are added at the end of the plan with their amounts as fixed cost; set the durations and links afterwards.")).color(theme::TEXT_DIM).size(11.5));
        crate::dialogs::error_line(ui, &st.error);
        let (ok, cancel) = crate::dialogs::buttons(ui, "Import");
        if ok {
            if list.is_empty() {
                st.error = Some(tr("There are no items to import: check the columns.").to_string());
            } else {
                done = Some(true);
            }
        } else if cancel {
            done = Some(false);
        }
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        done = Some(false);
    }
    if done == Some(true) {
        let list = items(&st.sheets[st.sheet].rows, &st.cols);
        app.checkpoint();
        let first = app.doc.tasks.len();
        let n = match add_to_project(&mut app.doc, &list, st.days, st.chain) {
            Ok(n) => n,
            Err(e) => {
                app.show_error(e);
                return Some(false);
            }
        };
        app.recalc();
        app.dirty = true;
        app.backstage = None;
        // show the first imported row and its bar
        if let Some(row) = app.visible_rows().iter().position(|&i| i == first) {
            app.sel_row = row;
        }
        app.scroll_to_task();
        app.status = trf("Imported {} rows from the BOQ", &[&n.to_string()]);
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Val {
        Val::Text(s.into())
    }
    fn n(v: f64) -> Val {
        Val::Num(v)
    }

    /// A typical Thai BOQ: title rows, a two-row header with merged ค่าวัสดุ / ค่าแรง, groups,
    /// items, a description line and subtotals.
    fn sample() -> Vec<Vec<Val>> {
        vec![
            vec![t("แบบแสดงรายการ ปริมาณงานและราคา")],
            vec![t("โครงการ อาคารสำนักงาน 3 ชั้น")],
            vec![t("ลำดับ"), t("รายการ"), t("จำนวน"), t("หน่วย"), t("ค่าวัสดุ"), Val::Empty, t("ค่าแรงงาน"), Val::Empty, t("รวมค่าวัสดุและค่าแรง"), t("หมายเหตุ")],
            vec![Val::Empty, Val::Empty, Val::Empty, Val::Empty, t("ราคาต่อหน่วย"), t("จำนวนเงิน"), t("ราคาต่อหน่วย"), t("จำนวนเงิน"), Val::Empty, Val::Empty],
            vec![t("1"), t("งานโครงสร้าง")],
            vec![t("1.1"), t("งานฐานราก")],
            vec![t("1.1.1"), t("คอนกรีตฐานราก 240 ksc."), n(45.0), t("ลบ.ม."), n(2100.0), n(94500.0), n(400.0), n(18000.0), n(112500.0)],
            vec![Val::Empty, t("- รวมค่าปั๊มคอนกรีต")],
            vec![t("1.1.2"), t("เหล็กเสริม DB16"), n(3200.0), t("กก."), n(24.0), n(76800.0), n(4.0), n(12800.0), n(89600.0)],
            vec![t("1.2"), t("งานเสาและคาน"), n(1.0), t("งาน"), Val::Empty, Val::Empty, Val::Empty, Val::Empty, t("350,000.00")],
            vec![Val::Empty, t("รวมงานโครงสร้าง"), Val::Empty, Val::Empty, Val::Empty, Val::Empty, Val::Empty, Val::Empty, n(552100.0)],
            vec![t("2"), t("งานสถาปัตยกรรม")],
            vec![t("2.1"), t("ก่ออิฐมอญครึ่งแผ่น"), n(850.0), t("ตร.ม."), n(180.0), n(153000.0), n(120.0), n(102000.0), n(255000.0)],
            vec![Val::Empty, t("ภาษีมูลค่าเพิ่ม 7%"), Val::Empty, Val::Empty, Val::Empty, Val::Empty, Val::Empty, Val::Empty, n(56497.0)],
            vec![Val::Empty, t("รวมทั้งสิ้น"), Val::Empty, Val::Empty, Val::Empty, Val::Empty, Val::Empty, Val::Empty, n(863597.0)],
        ]
    }

    #[test]
    fn finds_a_two_row_header() {
        let rows = sample();
        let c = detect(&rows).expect("header");
        assert_eq!((c.header_row, c.header_rows), (2, 2));
        assert_eq!((c.no, c.desc, c.qty, c.unit, c.amount), (Some(0), Some(1), Some(2), Some(3), Some(8)));
        assert_eq!((c.material, c.labour), (Some(5), Some(7)));
        let list = items(&rows, &c);
        let names: Vec<(&str, u8, bool)> = list.iter().map(|i| (i.name.as_str(), i.level, i.group)).collect();
        assert_eq!(
            names,
            vec![
                ("งานโครงสร้าง", 1, true),
                ("งานฐานราก", 2, true),
                ("คอนกรีตฐานราก 240 ksc.", 3, false),
                ("เหล็กเสริม DB16", 3, false),
                ("งานเสาและคาน", 2, false),
                ("งานสถาปัตยกรรม", 1, true),
                ("ก่ออิฐมอญครึ่งแผ่น", 2, false),
            ]
        );
        assert_eq!(list[2].amount, 112500.0);
        assert_eq!(list[2].notes, "- รวมค่าปั๊มคอนกรีต");
        assert_eq!(list[4].amount, 350000.0);
        let total: f64 = list.iter().map(|i| i.amount).sum();
        assert_eq!(total, 807100.0);
    }

    #[test]
    fn material_plus_labour_and_unnumbered() {
        // no total column, no numbers: groups are the rows without quantity
        let rows = vec![
            vec![t("รายการ"), t("ปริมาณ"), t("หน่วย"), t("ค่าวัสดุ (บาท)"), t("ค่าแรง (บาท)")],
            vec![t("งานหลังคา")],
            vec![t("โครงหลังคาเหล็ก"), n(120.0), t("ตร.ม."), n(60000.0), n(24000.0)],
            vec![t("มุงกระเบื้อง"), n(120.0), t("ตร.ม."), n(36000.0), n(12000.0)],
        ];
        let c = detect(&rows).unwrap();
        assert_eq!((c.desc, c.qty, c.unit, c.amount, c.material, c.labour), (Some(0), Some(1), Some(2), None, Some(3), Some(4)));
        let list = items(&rows, &c);
        assert_eq!(list.iter().map(|i| i.level).collect::<Vec<_>>(), vec![1, 2, 2]);
        assert_eq!(list[1].amount, 84000.0);
        // into a plan: a summary with two costed tasks
        let mut p = crate::model::Project::new();
        p.tasks.clear();
        assert_eq!(add_to_project(&mut p, &list, 3, true).unwrap(), 3);
        assert_eq!(p.tasks[1].fixed_cost, 84000.0);
        assert_eq!(p.tasks[2].predecessors[0].pred_uid, p.tasks[1].uid);
        assert!(p.tasks[0].predecessors.is_empty() && p.tasks[0].fixed_cost == 0.0);
        assert_eq!(p.tasks[1].duration, 3);
        assert_eq!(p.controls.boq.len(), 2);
        assert_eq!(p.controls.boq[0].quantity, Some(120.0));
        assert_eq!(p.controls.boq[0].contract_amount, 84000.0);
    }

    #[test]
    fn csv_and_xlsx() {
        let rows = read_csv("ลำดับ,รายการ,จำนวน,หน่วย,รวม\n1,\"งานดิน, ขุด\",10,ลบ.ม.,\"1,500\"\n".as_bytes());
        assert_eq!(rows[1][1], t("งานดิน, ขุด"));
        assert_eq!(rows[1][4].num(), Some(1500.0));
        // Windows-874 Thai
        let rows = read_csv(&[0xC3, 0xD2, 0xC2, 0xA1, 0xD2, 0xC3, b'\n']);
        assert_eq!(rows[0][0], t("รายการ"));
        assert_eq!(cell_ref("AB12"), Some((11, 27)));
        assert_eq!(numbering("1.2.3"), Some(3));
        assert_eq!(numbering("ก"), Some(1));
        assert_eq!(numbering("ก.2"), Some(2));
        assert_eq!(numbering("คอนกรีต"), None);
        // an .xlsx written by the program's own Excel export reads back
        let path = std::env::temp_dir().join(format!("neramit-boq-{}.xlsx", std::process::id()));
        let mut wb = rust_xlsxwriter::Workbook::new();
        let ws = wb.add_worksheet().set_name("BOQ").unwrap();
        ws.write_string(0, 0, "ลำดับ").unwrap();
        ws.write_string(0, 1, "รายการ").unwrap();
        ws.write_string(0, 2, "จำนวนเงิน").unwrap();
        ws.write_string(1, 0, "1").unwrap();
        ws.write_string(1, 1, "งานเตรียมการ & รื้อถอน").unwrap();
        ws.write_number(1, 2, 12500.5).unwrap();
        wb.save(&path).unwrap();
        let sheets = read_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(sheets[0].name, "BOQ");
        let c = detect(&sheets[0].rows).unwrap();
        let list = items(&sheets[0].rows, &c);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "งานเตรียมการ & รื้อถอน");
        assert_eq!(list[0].amount, 12500.5);
    }
}
