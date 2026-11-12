//! Lecture des entrées et validation, au comportement de Laravel :
//! - `$request->input()` fusionne query string et corps (JSON, formulaire ou multipart) ;
//! - middlewares globaux `TrimStrings` + `ConvertEmptyStringsToNull` : chaînes
//!   rognées, chaîne vide → null (sauf les mots de passe, non rognés) ;
//! - messages d'erreur : ceux de Laravel (anglais) sauf messages personnalisés.

use std::collections::HashMap;

use axum::extract::{FromRequest, Multipart, Request};
use axum::http::header::CONTENT_TYPE;
use axum::{Form, Json};
use serde_json::{Map, Value};

use crate::db::Q;
use crate::error::{AppError, AppResult};

/// Entrées fusionnées d'une requête.
#[derive(Debug, Default, Clone)]
pub struct Input(pub Map<String, Value>);

fn normalize(key: &str, v: Value) -> Value {
    match v {
        Value::String(s) if key.ends_with("password") => Value::String(s),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() { Value::Null } else { Value::String(t.to_string()) }
        }
        Value::Array(a) => Value::Array(a.into_iter().map(|x| normalize("", x)).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, x)| { let n = normalize(&k, x); (k, n) }).collect()),
        other => other,
    }
}

impl Input {
    /// Ajoute une paire de formulaire (`mots[]` → tableau `mots`).
    fn push_pair(&mut self, key: String, value: String) {
        let v = normalize(&key, Value::String(value));
        if let Some(base) = key.strip_suffix("[]").or_else(|| key.split_once('[').map(|(b, _)| b)) {
            let entry = self.0.entry(base.to_string()).or_insert_with(|| Value::Array(vec![]));
            if let Value::Array(a) = entry {
                a.push(v);
            }
        } else {
            self.0.insert(key, v);
        }
    }

    fn merge_query(&mut self, query: Option<&str>) {
        let Some(q) = query else { return };
        if let Ok(pairs) = serde_urlencoded_pairs(q) {
            for (k, v) in pairs {
                // Le corps l'emporte sur la query, comme `$request->input()`
                if !self.0.contains_key(k.trim_end_matches("[]")) {
                    self.push_pair(k, v);
                }
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key).filter(|v| !v.is_null())
    }

    /// Lecture d'un multipart : champs + premier fichier `image`.
    pub async fn from_multipart(mut mp: Multipart, query: Option<&str>) -> AppResult<(Input, Option<Vec<u8>>)> {
        let mut input = Input::default();
        let mut image = None;
        while let Some(field) = mp.next_field().await.map_err(|e| AppError::unprocessable(format!("Formulaire invalide : {e}")))? {
            let name = field.name().unwrap_or_default().to_string();
            if field.file_name().is_some() {
                let bytes = field.bytes().await.map_err(|_| AppError::unprocessable("Fichier illisible."))?;
                if name == "image" && !bytes.is_empty() {
                    image = Some(bytes.to_vec());
                }
            } else {
                let text = field.text().await.unwrap_or_default();
                input.push_pair(name, text);
            }
        }
        input.merge_query(query);
        Ok((input, image))
    }
}

fn serde_urlencoded_pairs(q: &str) -> Result<Vec<(String, String)>, ()> {
    // Décodage application/x-www-form-urlencoded minimal (+, %XX)
    fn decode(s: &str) -> String {
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'+' => out.push(b' '),
                b'%' if i + 2 < bytes.len() => {
                    match u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16) {
                        Ok(b) => {
                            out.push(b);
                            i += 2;
                        }
                        Err(_) => out.push(b'%'),
                    }
                }
                b => out.push(b),
            }
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }
    Ok(q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (decode(k), decode(v)),
            None => (decode(p), String::new()),
        })
        .collect())
}

impl<S: Send + Sync> FromRequest<S> for Input {
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let query = req.uri().query().map(str::to_string);
        let ctype = req.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or_default().to_string();
        let mut input = Input::default();
        if ctype.starts_with("application/json") {
            let Json(v) = Json::<Value>::from_request(req, state)
                .await
                .map_err(|_| AppError::unprocessable("Corps JSON invalide."))?;
            if let Value::Object(o) = normalize("", v) {
                input.0 = o;
            }
        } else if ctype.starts_with("application/x-www-form-urlencoded") {
            let Form(pairs) = Form::<Vec<(String, String)>>::from_request(req, state)
                .await
                .map_err(|_| AppError::unprocessable("Formulaire invalide."))?;
            for (k, v) in pairs {
                input.push_pair(k, v);
            }
        }
        input.merge_query(query.as_deref());
        Ok(input)
    }
}

/// Nom d'attribut affiché par Laravel : `player_id` → `player id`.
fn attr(field: &str) -> String {
    field.replace('_', " ")
}

/// Validateur au comportement de Laravel (une erreur par champ, dans l'ordre).
pub struct V<'a> {
    input: &'a Input,
    errors: Vec<(String, String)>,
    custom: HashMap<String, String>,
}

impl<'a> V<'a> {
    pub fn new(input: &'a Input) -> Self {
        Self { input, errors: vec![], custom: HashMap::new() }
    }

    /// Messages personnalisés : `("pseudo.required", "Le pseudo est requis.")`.
    pub fn messages(mut self, m: &[(&str, &str)]) -> Self {
        self.custom.extend(m.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        self
    }

    fn has_error(&self, field: &str) -> bool {
        self.errors.iter().any(|(f, _)| f == field)
    }

    pub fn fail(&mut self, field: &str, rule: &str, default: String) {
        if self.has_error(field) {
            return;
        }
        let msg = self.custom.get(&format!("{field}.{rule}")).cloned().unwrap_or(default);
        self.errors.push((field.to_string(), msg));
    }

    fn present(&self, field: &str) -> Option<&'a Value> {
        let v = self.input.get(field)?;
        if matches!(v, Value::Array(a) if a.is_empty()) { None } else { Some(v) }
    }

    fn required(&mut self, field: &str) -> Option<&'a Value> {
        let v = self.present(field);
        if v.is_none() {
            self.fail(field, "required", format!("The {} field is required.", attr(field)));
        }
        v
    }

    fn as_int(&mut self, field: &str, v: &Value) -> Option<i64> {
        let n = match v {
            Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)),
            Value::String(s) => s.parse::<i64>().ok(),
            _ => None,
        };
        if n.is_none() {
            self.fail(field, "integer", format!("The {} field must be an integer.", attr(field)));
        }
        n
    }

    pub fn req_int(&mut self, field: &str) -> Option<i64> {
        let v = self.required(field)?;
        self.as_int(field, v)
    }

    pub fn opt_int(&mut self, field: &str) -> Option<i64> {
        let v = self.present(field)?;
        self.as_int(field, v)
    }

    pub fn min_int(&mut self, field: &str, n: Option<i64>, min: i64) {
        if n.is_some_and(|x| x < min) {
            self.fail(field, "min", format!("The {} field must be at least {min}.", attr(field)));
        }
    }

    pub fn max_int(&mut self, field: &str, n: Option<i64>, max: i64) {
        if n.is_some_and(|x| x > max) {
            self.fail(field, "max", format!("The {} field must not be greater than {max}.", attr(field)));
        }
    }

    fn as_bool(&mut self, field: &str, v: &Value) -> Option<bool> {
        let b = match v {
            Value::Bool(b) => Some(*b),
            Value::Number(n) if n.as_i64() == Some(1) => Some(true),
            Value::Number(n) if n.as_i64() == Some(0) => Some(false),
            Value::String(s) if s == "1" => Some(true),
            Value::String(s) if s == "0" => Some(false),
            _ => None,
        };
        if b.is_none() {
            self.fail(field, "boolean", format!("The {} field must be true or false.", attr(field)));
        }
        b
    }

    pub fn req_bool(&mut self, field: &str) -> Option<bool> {
        let v = self.required(field)?;
        self.as_bool(field, v)
    }

    pub fn opt_bool(&mut self, field: &str) -> Option<bool> {
        let v = self.present(field)?;
        self.as_bool(field, v)
    }

    fn as_str(&mut self, field: &str, v: &Value, min: Option<usize>, max: Option<usize>) -> Option<String> {
        let Value::String(s) = v else {
            self.fail(field, "string", format!("The {} field must be a string.", attr(field)));
            return None;
        };
        let len = s.chars().count();
        if let Some(m) = min.filter(|m| len < *m) {
            self.fail(field, "min", format!("The {} field must be at least {m} characters.", attr(field)));
            return None;
        }
        if let Some(m) = max.filter(|m| len > *m) {
            self.fail(field, "max", format!("The {} field must not be greater than {m} characters.", attr(field)));
            return None;
        }
        Some(s.clone())
    }

    pub fn req_str(&mut self, field: &str, min: Option<usize>, max: Option<usize>) -> Option<String> {
        let v = self.required(field)?;
        self.as_str(field, v, min, max)
    }

    pub fn opt_str(&mut self, field: &str, min: Option<usize>, max: Option<usize>) -> Option<String> {
        let v = self.present(field)?;
        self.as_str(field, v, min, max)
    }

    /// Chaîne de longueur exacte.
    pub fn size_str(&mut self, field: &str, s: Option<&String>, size: usize) {
        if s.is_some_and(|s| s.chars().count() != size) {
            self.fail(field, "size", format!("The {} field must be {size} characters.", attr(field)));
        }
    }

    pub fn in_list(&mut self, field: &str, value: Option<&String>, allowed: &[&str]) -> bool {
        match value {
            Some(v) if !allowed.contains(&v.as_str()) => {
                self.fail(field, "in", format!("The selected {} is invalid.", attr(field)));
                false
            }
            _ => true,
        }
    }

    /// Tableau de chaînes (`forbidden_words`), taille exacte facultative.
    pub fn req_str_array(&mut self, field: &str, size: Option<usize>, item_max: usize) -> Option<Vec<String>> {
        let v = self.required(field)?;
        let Value::Array(items) = v else {
            self.fail(field, "array", format!("The {} field must be an array.", attr(field)));
            return None;
        };
        if let Some(n) = size.filter(|n| items.len() != *n) {
            self.fail(field, "size", format!("The {} field must contain {n} items.", attr(field)));
            return None;
        }
        let mut out = vec![];
        for (i, item) in items.iter().enumerate() {
            let key = format!("{field}.{i}");
            match item {
                Value::String(s) if s.chars().count() <= item_max => out.push(s.clone()),
                Value::String(_) => self.fail(&key, "max", format!("The {key} field must not be greater than {item_max} characters.")),
                Value::Null => self.fail(&key, "required", format!("The {key} field is required.")),
                _ => self.fail(&key, "string", format!("The {key} field must be a string.")),
            }
        }
        (out.len() == items.len()).then_some(out)
    }

    /// Tableau d'entiers (`character_ids`).
    pub fn req_int_array(&mut self, field: &str) -> Option<Vec<i64>> {
        let v = self.required(field)?;
        let Value::Array(items) = v else {
            self.fail(field, "array", format!("The {} field must be an array.", attr(field)));
            return None;
        };
        let mut out = vec![];
        for (i, item) in items.iter().enumerate() {
            let key = format!("{field}.{i}");
            match self.as_int(&key, item) {
                Some(n) => out.push(n),
                None => return None,
            }
        }
        Some(out)
    }

    /// Règle `exists` : `sql` doit renvoyer une ligne pour `id`.
    pub fn exists(&mut self, conn: &impl Q, field: &str, id: Option<i64>, sql: &str) -> AppResult<Option<i64>> {
        let Some(id) = id else { return Ok(None) };
        if conn.exists(sql, [id])? {
            Ok(Some(id))
        } else {
            self.fail(field, "exists", format!("The selected {} is invalid.", attr(field)));
            Ok(None)
        }
    }

    pub fn player(&mut self, conn: &impl Q, field: &str) -> AppResult<Option<i64>> {
        let id = self.req_int(field);
        self.exists(conn, field, id, "SELECT 1 FROM players WHERE id = ?1")
    }

    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn finish(self) -> AppResult<()> {
        if self.errors.is_empty() { Ok(()) } else { Err(AppError::Validation(self.errors)) }
    }
}
