//! A tiny MCP server (stdio, JSON-RPC) that the app starts for Claude Code during a
//! test run. It offers two tools so Claude reports each checklist result the moment
//! it is known. The app reads the tool calls live from Claude's output stream, so
//! the server itself only has to acknowledge them.

use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn tools() -> Value {
    json!([
        {
            "name": "ergebnis_melden",
            "description": "Meldet das Ergebnis EINES Prüfpunkts an die Abnahme-App. Sofort aufrufen, sobald ein Prüfpunkt bewertet ist – für jeden Prüfpunkt genau einmal (bei Nachtests erneut).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "ID des Prüfpunkts, z. B. UC1-01"},
                    "status": {"type": "string", "enum": ["passed", "failed", "blocked"]},
                    "note": {"type": "string", "description": "Was konkret beobachtet wurde (kurz, nachvollziehbar, auf Deutsch)"},
                    "evidence": {"type": "array", "items": {"type": "string"}, "description": "Dateinamen von Screenshots, z. B. UC1-01.png"},
                    "blocker": {"type": "string", "description": "PFLICHT bei blocked: was genau den Test verhindert hat (z. B. Abhängigkeit von UC1-03, fehlende Testdaten/Zugänge, Fehlermeldung) und was nötig wäre, um ihn durchzuführen"}
                },
                "required": ["id", "status", "note"]
            }
        },
        {
            "name": "zusammenfassung_melden",
            "description": "Meldet am Ende des Testlaufs die Gesamtbewertung und eine Abnahme-Empfehlung an die App.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "summary": {"type": "string", "description": "Zusammenfassung für das Abnahmeprotokoll (Deutsch)"},
                    "recommendation": {"type": "string", "enum": ["accept", "accept_with_conditions", "reject"]},
                    "conditions": {"type": "string", "description": "Auflagen / offene Punkte, falls vorhanden"}
                },
                "required": ["summary", "recommendation"]
            }
        }
    ])
}

pub fn run_stdio() {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(line.trim()) else { continue };
        let Some(id) = msg.get("id").cloned() else { continue }; // notification
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let reply = match method {
            "initialize" => {
                let ver = msg
                    .pointer("/params/protocolVersion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("2024-11-05")
                    .to_string();
                json!({"jsonrpc":"2.0","id":id,"result":{
                    "protocolVersion": ver,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "abnahme", "version": "1.0"}
                }})
            }
            "ping" => json!({"jsonrpc":"2.0","id":id,"result":{}}),
            "tools/list" => json!({"jsonrpc":"2.0","id":id,"result":{"tools": tools()}}),
            "tools/call" => {
                let name = msg.pointer("/params/name").and_then(|v| v.as_str()).unwrap_or("");
                let text = match name {
                    "ergebnis_melden" => {
                        let pid = msg.pointer("/params/arguments/id").and_then(|v| v.as_str()).unwrap_or("?");
                        format!("Ergebnis für {pid} gespeichert.")
                    }
                    "zusammenfassung_melden" => "Zusammenfassung gespeichert.".to_string(),
                    _ => "Unbekanntes Werkzeug.".to_string(),
                };
                json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]}})
            }
            _ => json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not found"}}),
        };
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
}
