//! Rascunho (como na Escrita do Autor Total online): o trecho selecionado vai para um documento à
//! parte, para lapidar; depois volta para o mesmo lugar do texto.
//!
//! O lugar de origem fica marcado com um marcador oculto (`_Rascunho_…`) e o documento do rascunho
//! guarda o nome dele numa propriedade (`AutorTotalRascunho`). "Devolver ao texto" procura o
//! marcador nos documentos abertos e troca o trecho pelo que está no rascunho (Ctrl+Z desfaz, no
//! texto de origem).

use serde_json::json;
use wordcraft_engine::Selection;

use crate::WordApp;

/// Propriedade do documento de rascunho com o nome do marcador de origem.
pub const PROP: &str = "AutorTotalRascunho";
/// Propriedade com o começo do trecho (para o título e a faixa).
const PROP_TITULO: &str = "AutorTotalRascunhoTitulo";

/// O rascunho aberto: (marcador de origem, título).
pub fn deste(app: &WordApp) -> Option<(String, String)> {
    let m = app.session.doc.custom_prop(PROP)?.to_string();
    let t = app.session.doc.custom_prop(PROP_TITULO).unwrap_or("Trecho").to_string();
    Some((m, t))
}

fn novo_id() -> String {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    // base 36, os últimos 10 dígitos bastam para não repetir num mesmo texto
    let mut x = n % 36u128.pow(10);
    let mut s = String::new();
    while x > 0 {
        let d = (x % 36) as u32;
        s.insert(0, char::from_digit(d, 36).unwrap_or('0'));
        x /= 36;
    }
    s
}

/// Leva a seleção (ou o parágrafo do cursor) para um rascunho novo.
pub fn levar(app: &mut WordApp) -> Result<(), String> {
    if deste(app).is_some() {
        return Err("Este documento já é um rascunho: selecione o trecho no texto original.".into());
    }
    if app.session.sel.is_collapsed() {
        app.run("select.paragraph", json!({}))?;
    }
    let (a, b) = app.session.sel.ordered();
    let texto = app.session.doc.para_at(&a).and_then(|p| p.text.get(a.off..)).unwrap_or("").to_string();
    let palavras: Vec<&str> = texto.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).take(7).collect();
    let mut titulo = palavras.iter().take(6).copied().collect::<Vec<_>>().join(" ");
    if palavras.len() > 6 {
        titulo.push('…');
    }
    if titulo.is_empty() {
        titulo = "Trecho".into();
    }
    let marca = format!("_Rascunho_{}", novo_id());
    let origem = app.session.document_id();
    // Marca o lugar e copia o trecho.
    app.session.sel = Selection { anchor: a, focus: b };
    app.run("insert.bookmark", json!({"name": marca}))?;
    if let Some((x, y)) = wordcraft_engine::cmd::insert::bookmark_range(&app.session.doc, &marca) {
        app.session.sel = Selection { anchor: x, focus: y };
    }
    app.run("edit.copy", json!({}))?;
    // O rascunho: um documento novo com o trecho.
    app.run("file.new", json!({}))?;
    if app.session.document_id() == origem {
        return Err("Não consegui abrir o documento do rascunho.".into());
    }
    app.run("edit.paste", json!({}))?;
    app.session.doc.set_custom_prop(PROP, &marca);
    app.session.doc.set_custom_prop(PROP_TITULO, &titulo);
    app.session.file_name = Some(format!("Rascunho — {titulo}"));
    app.status(format!("Trecho no Rascunho. Lapide à vontade e use Devolver ao texto quando estiver pronto."));
    Ok(())
}

/// Troca o trecho de origem pelo conteúdo do rascunho.
pub fn devolver(app: &mut WordApp) -> Result<(), String> {
    let Some((marca, _)) = deste(app) else {
        return Err("Este documento não é um rascunho.".into());
    };
    let rascunho = app.session.document_id();
    app.run("select.all", json!({}))?;
    app.run("edit.copy", json!({}))?;
    let outros: Vec<u64> = app.session.documents().into_iter().map(|d| d.id).filter(|id| *id != rascunho).collect();
    for id in outros {
        if app.session.activate(id).is_err() {
            continue;
        }
        if let Some((x, y)) = wordcraft_engine::cmd::insert::bookmark_range(&app.session.doc, &marca) {
            app.session.sel = Selection { anchor: x.clone(), focus: y };
            app.run("edit.paste", json!({}))?;
            // O trecho novo continua marcado, para poder levar de novo ou devolver outra versão.
            app.status("Trecho devolvido ao texto. Ctrl+Z aqui volta a versão anterior.".to_string());
            app.sync_document();
            return Ok(());
        }
    }
    let _ = app.session.activate(rascunho);
    app.sync_document();
    Err("Não achei o lugar de origem desse trecho: o texto original precisa estar aberto (e o trecho não pode ter sido apagado).".into())
}

/// Vai ao lugar de origem do rascunho, no texto.
pub fn ir_ao_texto(app: &mut WordApp) -> Result<(), String> {
    let Some((marca, _)) = deste(app) else { return Err("Este documento não é um rascunho.".into()) };
    let rascunho = app.session.document_id();
    let outros: Vec<u64> = app.session.documents().into_iter().map(|d| d.id).filter(|id| *id != rascunho).collect();
    for id in outros {
        if app.session.activate(id).is_err() {
            continue;
        }
        if let Some((x, y)) = wordcraft_engine::cmd::insert::bookmark_range(&app.session.doc, &marca) {
            app.session.sel = Selection { anchor: x, focus: y };
            app.sync_document();
            return Ok(());
        }
    }
    let _ = app.session.activate(rascunho);
    app.sync_document();
    Err("O texto original não está aberto.".into())
}

/// Faixa no topo de um documento de rascunho.
pub fn faixa(app: &mut WordApp, ui: &mut egui::Ui) {
    let Some((_, titulo)) = deste(app) else { return };
    let t = crate::theme::Tokens::get(ui.ctx());
    egui::Frame::NONE.fill(t.accent.gamma_multiply(0.18)).inner_margin(egui::Margin::symmetric(12, 6)).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("Rascunho de «{titulo}»")).strong());
            ui.label("Lapide à vontade; o texto original fica como estava até você devolver.");
            if ui.button("↩ Devolver ao texto").clicked()
                && let Err(e) = devolver(app)
            {
                app.status(e);
            }
            if ui.button("Ir ao trecho no texto").clicked()
                && let Err(e) = ir_ao_texto(app)
            {
                app.status(e);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use wordcraft_engine::Session;

    fn texto(app: &WordApp) -> String {
        app.session.doc.plain_text(wordcraft_doc::StoryRef::Body)
    }

    #[test]
    fn leva_o_paragrafo_e_devolve_ao_mesmo_lugar() {
        let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default());
        app.run("text.insert", json!({"text": "Primeiro parágrafo."})).unwrap();
        app.run("text.newParagraph", json!({})).unwrap();
        app.run("text.insert", json!({"text": "Segundo parágrafo para lapidar."})).unwrap();
        app.run("text.newParagraph", json!({})).unwrap();
        app.run("text.insert", json!({"text": "Terceiro."})).unwrap();
        // cursor no segundo parágrafo
        app.run("caret.up", json!({})).unwrap();
        let origem = app.session.document_id();
        levar(&mut app).unwrap();
        assert_ne!(app.session.document_id(), origem);
        assert!(texto(&app).contains("Segundo parágrafo para lapidar."), "{}", texto(&app));
        assert!(!texto(&app).contains("Primeiro"), "{}", texto(&app));
        assert!(deste(&app).is_some());
        // lapida no rascunho
        app.run("select.all", json!({})).unwrap();
        app.run("text.insert", json!({"text": "Segundo parágrafo, agora lapidado."})).unwrap();
        devolver(&mut app).unwrap();
        assert_eq!(app.session.document_id(), origem);
        let t = texto(&app);
        assert!(t.contains("Primeiro parágrafo."), "{t}");
        assert!(t.contains("Segundo parágrafo, agora lapidado."), "{t}");
        assert!(!t.contains("para lapidar"), "{t}");
        assert!(t.contains("Terceiro."), "{t}");
        let pos_prim = t.find("Primeiro").unwrap();
        let pos_seg = t.find("agora lapidado").unwrap();
        let pos_ter = t.find("Terceiro").unwrap();
        assert!(pos_prim < pos_seg && pos_seg < pos_ter, "{t}");
    }

    #[test]
    fn devolver_sem_ser_rascunho_e_erro() {
        let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default());
        assert!(devolver(&mut app).is_err());
    }
}
