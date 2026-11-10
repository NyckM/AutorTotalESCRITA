//! Autor Total Escrita: o Mentor do Autor Total dentro do editor.
//!
//! Um painel à direita do documento (F12, ou a aba "✦ Mentor" na borda direita) com a conversa
//! com o Mentor online: entra com a conta do Autor Total (a mesma do app, com a verificação em duas etapas),
//! usa a seleção e o documento abertos aqui, insere a resposta no texto ou como comentário na margem, e
//! manda o documento para uma história nova no Autor Total online. Nenhuma chave secreta fica no programa:
//! só a chave pública do projeto (a mesma da página do Mentor) e o login da própria pessoa.
//!
//! A rede roda numa thread por pedido; a interface só lê os eventos que chegam pelo canal.

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

use egui::text::LayoutJob;
use egui::{Align, Color32, CornerRadius, Key, Layout, Margin, RichText, Stroke, TextFormat, Ui, vec2};
use serde_json::{Value, json};

use crate::WordApp;
use crate::theme::{Tokens, regular, semibold};

const SB_URL: &str = "https://wbwlhxwfhrgaikettdev.supabase.co";
/// Chave pública (publishable) do projeto: a mesma que está na página do Mentor; não dá acesso a nada sem login.
const SB_KEY: &str = "sb_publishable_Rgg8mN05JXMumeE5amE5hA_G7PI94hr";
const SITE: &str = "https://autortotal.com.br/";
const CRIAR_CONTA: &str = "https://autortotal.com.br/";
const LARGURA: f32 = 360.0;
const CRITICAS: [(&str, &str); 3] = [("leve", "Crítica leve"), ("media", "Crítica média"), ("rigorosa", "Crítica rigorosa")];

#[derive(Clone, Default)]
struct Sessao {
    access: String,
    refresh: String,
    email: String,
    user_id: String,
    expira: i64,
}

enum Evento {
    /// Login, renovação ou código confirmado.
    Sessao(Sessao),
    /// A senha estava certa, mas a conta tem verificação em duas etapas: falta o código.
    PrecisaCodigo(Sessao, String),
    Delta(String),
    Fim,
    Erro(String),
    /// A história nova foi criada no Autor Total online (id).
    Enviado(String),
    /// A sessão guardada não vale mais: volta para a tela de entrar.
    Saiu(String),
}

struct Msg {
    eu: bool,
    texto: String,
    trecho: String,
    erro: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tarefa {
    Nada,
    Entrando,
    Respondendo,
    Enviando,
}

struct Mentor {
    aberto: bool,
    iniciado: bool,
    sessao: Option<Sessao>,
    pendente: Option<(Sessao, String)>,
    email: String,
    senha: String,
    codigo: String,
    msgs: Vec<Msg>,
    hist: Vec<(String, String)>,
    pergunta: String,
    trecho: String,
    ler_doc: bool,
    critica: usize,
    tarefa: Tarefa,
    erro_login: String,
    aviso: Option<(String, f64)>,
    rx: Option<Receiver<Evento>>,
    rolar: bool,
    logo: Option<egui::TextureHandle>,
}

impl Mentor {
    fn novo() -> Self {
        Mentor {
            aberto: false,
            iniciado: false,
            sessao: None,
            pendente: None,
            email: String::new(),
            senha: String::new(),
            codigo: String::new(),
            msgs: Vec::new(),
            hist: Vec::new(),
            pergunta: String::new(),
            trecho: String::new(),
            ler_doc: true,
            critica: 1,
            tarefa: Tarefa::Nada,
            erro_login: String::new(),
            aviso: None,
            rx: None,
            rolar: false,
            logo: None,
        }
    }
}

thread_local! {
    static M: RefCell<Mentor> = RefCell::new(Mentor::novo());
}

/// Abre ou fecha o painel (atalho e aba da borda).
pub fn alternar() {
    M.with(|m| {
        let mut m = m.borrow_mut();
        m.aberto = !m.aberto;
    });
}

// ---------- cores do Autor Total ----------

struct Cores {
    ac: Color32,
    ac_ink: Color32,
    ac_suave: Color32,
    fundo: Color32,
    bolha: Color32,
    eu: Color32,
    linha: Color32,
    texto: Color32,
    fraco: Color32,
    erro: Color32,
}

fn cores(t: &Tokens) -> Cores {
    if t.dark {
        // Noite: amarelo/laranja sobre quase preto
        Cores {
            ac: Color32::from_rgb(0xff, 0xc9, 0x28),
            ac_ink: Color32::from_rgb(0x1a, 0x12, 0x06),
            ac_suave: Color32::from_rgba_unmultiplied(0xff, 0xc9, 0x28, 0x22),
            fundo: Color32::from_rgb(0x14, 0x0f, 0x0d),
            bolha: Color32::from_rgb(0x1f, 0x18, 0x14),
            eu: Color32::from_rgb(0x33, 0x27, 0x14),
            linha: Color32::from_rgba_unmultiplied(0xff, 0xec, 0xd2, 0x22),
            texto: Color32::from_rgb(0xf6, 0xee, 0xe0),
            fraco: Color32::from_rgba_unmultiplied(0xf6, 0xee, 0xe0, 0x88),
            erro: Color32::from_rgb(0xff, 0x8a, 0x7a),
        }
    } else {
        // Papel: roxo sobre creme
        Cores {
            ac: Color32::from_rgb(0x5a, 0x3f, 0xd6),
            ac_ink: Color32::WHITE,
            ac_suave: Color32::from_rgba_unmultiplied(0x5a, 0x3f, 0xd6, 0x18),
            fundo: Color32::from_rgb(0xf4, 0xf1, 0xec),
            bolha: Color32::WHITE,
            eu: Color32::from_rgb(0xeb, 0xe6, 0xfb),
            linha: Color32::from_rgba_unmultiplied(0x28, 0x1c, 0x0e, 0x22),
            texto: Color32::from_rgb(0x1d, 0x18, 0x13),
            fraco: Color32::from_rgba_unmultiplied(0x1d, 0x18, 0x13, 0x88),
            erro: Color32::from_rgb(0xb3, 0x26, 0x1e),
        }
    }
}

fn botao(ui: &mut Ui, c: &Cores, txt: &str, primario: bool) -> egui::Response {
    let rt = RichText::new(txt).font(if primario { semibold(13.0) } else { regular(12.5) }).color(if primario { c.ac_ink } else { c.texto });
    let b = egui::Button::new(rt)
        .fill(if primario { c.ac } else { Color32::TRANSPARENT })
        .stroke(if primario { Stroke::NONE } else { Stroke::new(1.0, c.linha) })
        .corner_radius(CornerRadius::same(14))
        .min_size(vec2(0.0, if primario { 30.0 } else { 26.0 }));
    ui.add(b)
}

/// A estrela do Autor Total (o ícone do programa), do tamanho pedido.
fn estrela(ui: &mut Ui, m: &mut Mentor, lado: f32) -> egui::Response {
    if m.logo.is_none() {
        let png: &[u8] = include_bytes!("../../../assets/app-icon/hicolor/64x64/apps/ai.storyteller.wordcraft.png");
        if let Ok(img) = image::load_from_memory(png) {
            let rgba = img.to_rgba8();
            let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
            m.logo = Some(ui.ctx().load_texture("at_mentor_estrela", ci, egui::TextureOptions::LINEAR));
        }
    }
    match &m.logo {
        Some(t) => ui.add(egui::Image::new(egui::load::SizedTexture::new(t.id(), vec2(lado, lado))).sense(egui::Sense::click())),
        None => ui.label(RichText::new("*").font(semibold(lado))),
    }
}

/// Botão redondo com uma seta para cima (a fonte da interface não tem o caractere da seta).
fn botao_enviar(ui: &mut Ui, c: &Cores, ativo: bool) -> egui::Response {
    let b = egui::Button::new("").fill(c.ac).corner_radius(CornerRadius::same(17)).min_size(vec2(34.0, 34.0));
    let r = ui.add_enabled(ativo, b);
    let ct = r.rect.center();
    let st = Stroke::new(2.2, c.ac_ink);
    let p = ui.painter();
    p.line_segment([egui::pos2(ct.x, ct.y + 7.0), egui::pos2(ct.x, ct.y - 7.0)], st);
    p.line_segment([egui::pos2(ct.x - 6.0, ct.y - 1.0), egui::pos2(ct.x, ct.y - 7.0)], st);
    p.line_segment([egui::pos2(ct.x + 6.0, ct.y - 1.0), egui::pos2(ct.x, ct.y - 7.0)], st);
    r
}

// ---------- painel ----------

/// Desenha o painel (ou a aba fechada) na borda direita. Chamado antes da área do documento.
pub fn show(app: &mut WordApp, ui: &mut Ui, t: Tokens) {
    let ctx = ui.ctx().clone();
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F12)) {
        alternar();
    }
    M.with(|cel| {
        let mut m = cel.borrow_mut();
        if !m.iniciado {
            m.iniciado = true;
            // Só nas compilações de teste: abre com uma conversa de exemplo, sem rede (para conferir o visual).
            #[cfg(debug_assertions)]
            if std::env::var_os("AT_MENTOR_DEMO").is_some() {
                m.aberto = true;
                m.sessao = Some(Sessao { email: "demo@autortotal.com.br".into(), expira: i64::MAX, ..Default::default() });
                m.msgs.push(Msg { eu: true, texto: "Esse começo prende o leitor?".into(), trecho: "Every Thursday evening the old print shop on Alder Street fills with people who make things.".into(), erro: false });
                m.msgs.push(Msg { eu: false, texto: "Prende, sim: a imagem é concreta e já mostra **movimento**.\n\n- Diga logo quem narra.\n- Mostre um conflito na segunda frase.\n\n> Um lugar cheio pede alguém que destoe dele.\n\nExperimente começar pelo personagem que chega atrasado.".into(), trecho: String::new(), erro: false });
            }
            if let Some(r) = ler_refresh() {
                m.tarefa = Tarefa::Entrando;
                let (tx, rx) = channel();
                m.rx = Some(rx);
                trabalhar(&ctx, tx, move |tx| match renovar(&r) {
                    Ok(s) => {
                        let _ = tx.send(Evento::Sessao(s));
                    }
                    Err(_) => {
                        let _ = tx.send(Evento::Saiu(String::new()));
                    }
                });
            }
        }
        receber(&mut m);
        let c = cores(&t);
        if !m.aberto {
            egui::Panel::right("at_mentor_aba").exact_size(30.0).resizable(false).show_separator_line(true)
                .frame(egui::Frame::NONE.fill(t.panel)).show(ui, |ui| {
                    ui.add_space(12.0);
                    ui.vertical_centered(|ui| {
                        let r = estrela(ui, &mut *m, 20.0).on_hover_text("Abrir o Mentor do Autor Total (F12)");
                        let r2 = ui.add(egui::Label::new(RichText::new("M\ne\nn\nt\no\nr").font(semibold(11.0)).color(t.text_dim)).sense(egui::Sense::click()));
                        if r.clicked() || r2.clicked() {
                            m.aberto = true;
                        }
                    });
                });
            return;
        }
        egui::Panel::right("at_mentor").default_size(LARGURA).size_range(280.0..=620.0).resizable(true)
            .frame(egui::Frame::NONE.fill(c.fundo).inner_margin(Margin::same(0)))
            .show(ui, |ui| painel(app, ui, &mut m, &c));
        if m.tarefa != Tarefa::Nada || m.aviso.is_some() {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
    });
}

fn topo(ui: &mut Ui, m: &mut Mentor, c: &Cores) {
    egui::Frame::NONE.inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
        ui.horizontal(|ui| {
            estrela(ui, m, 22.0);
            ui.label(RichText::new("Mentor").font(semibold(17.0)).color(c.texto));
            egui::Frame::NONE.fill(c.ac_suave).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(7, 2)).show(ui, |ui| {
                ui.label(RichText::new("Autor Total").font(regular(11.0)).color(c.ac));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("×").font(regular(17.0)).color(c.fraco)).frame(false)).on_hover_text("Fechar o painel (F12)").clicked() {
                    m.aberto = false;
                }
                if m.sessao.is_some() {
                    if ui.add(egui::Button::new(RichText::new("Sair").font(regular(12.0)).color(c.fraco)).frame(false)).clicked() {
                        m.sessao = None;
                        m.msgs.clear();
                        m.hist.clear();
                        apagar_refresh();
                    }
                    if ui.add(egui::Button::new(RichText::new("Nova").font(regular(12.0)).color(c.fraco)).frame(false))
                        .on_hover_text("Começar uma conversa nova").clicked() && m.tarefa != Tarefa::Respondendo {
                        m.msgs.clear();
                        m.hist.clear();
                    }
                }
            });
        });
    });
    let r = ui.max_rect();
    ui.painter().hline(r.x_range(), ui.cursor().top(), Stroke::new(1.0, c.linha));
}

fn painel(app: &mut WordApp, ui: &mut Ui, m: &mut Mentor, c: &Cores) {
    topo(ui, m, c);
    if m.sessao.is_none() {
        entrar(ui, m, c);
        return;
    }
    // compositor embaixo, conversa no meio
    egui::Panel::bottom("at_mentor_compor").resizable(false).show_separator_line(true)
        .frame(egui::Frame::NONE.fill(c.fundo).inner_margin(Margin::symmetric(10, 8)))
        .show(ui, |ui| compor(app, ui, m, c));
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(c.fundo)).show(ui, |ui| conversa(app, ui, m, c));
}

fn entrar(ui: &mut Ui, m: &mut Mentor, c: &Cores) {
    egui::Frame::NONE.inner_margin(Margin::symmetric(18, 22)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(RichText::new("Entre com a sua conta do Autor Total").font(semibold(19.0)).color(c.texto));
        ui.label(RichText::new("O Mentor usa o mesmo login, a mesma cota e o mesmo jeito de ler do app online.").font(regular(13.0)).color(c.fraco));
        ui.add_space(6.0);
        let ocupado = m.tarefa == Tarefa::Entrando;
        let mut ir = false;
        if m.pendente.is_none() {
            ui.label(RichText::new("E-mail").font(regular(12.0)).color(c.fraco));
            ui.add_enabled(!ocupado, egui::TextEdit::singleline(&mut m.email).desired_width(f32::INFINITY).margin(Margin::symmetric(9, 7)));
            ui.label(RichText::new("Senha").font(regular(12.0)).color(c.fraco));
            let r = ui.add_enabled(!ocupado, egui::TextEdit::singleline(&mut m.senha).password(true).desired_width(f32::INFINITY).margin(Margin::symmetric(9, 7)));
            ir |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            ui.add_space(4.0);
            ui.add_enabled_ui(!ocupado, |ui| {
                ir |= botao(ui, c, if ocupado { "Entrando…" } else { "Entrar" }, true).clicked();
            });
            if ir && !ocupado && !m.email.trim().is_empty() && !m.senha.is_empty() {
                let (email, senha) = (m.email.trim().to_string(), std::mem::take(&mut m.senha));
                m.erro_login.clear();
                m.tarefa = Tarefa::Entrando;
                let (tx, rx) = channel();
                m.rx = Some(rx);
                trabalhar(ui.ctx(), tx, move |tx| {
                    let _ = tx.send(match login(&email, &senha) {
                        Ok((s, Some(fator))) => Evento::PrecisaCodigo(s, fator),
                        Ok((s, None)) => Evento::Sessao(s),
                        Err(e) => Evento::Erro(e),
                    });
                });
            }
        } else {
            ui.label(RichText::new("Código de verificação (6 números do seu aplicativo autenticador)").font(regular(12.0)).color(c.fraco));
            let r = ui.add_enabled(!ocupado, egui::TextEdit::singleline(&mut m.codigo).char_limit(6).desired_width(f32::INFINITY).margin(Margin::symmetric(9, 7)));
            ir |= r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            ui.add_enabled_ui(!ocupado, |ui| {
                ir |= botao(ui, c, "Confirmar", true).clicked();
            });
            if ir && !ocupado && m.codigo.trim().len() == 6 {
                if let Some((s, fator)) = m.pendente.clone() {
                    let codigo = m.codigo.trim().to_string();
                    m.tarefa = Tarefa::Entrando;
                    let (tx, rx) = channel();
                    m.rx = Some(rx);
                    trabalhar(ui.ctx(), tx, move |tx| {
                        let _ = tx.send(match verificar_codigo(&s, &fator, &codigo) {
                            Ok(s) => Evento::Sessao(s),
                            Err(e) => Evento::Erro(e),
                        });
                    });
                }
            }
        }
        if !m.erro_login.is_empty() {
            ui.label(RichText::new(&m.erro_login).font(regular(12.5)).color(c.erro));
        }
        ui.add_space(6.0);
        ui.hyperlink_to(RichText::new("Ainda não tem conta? Criar no Autor Total").font(regular(12.5)).color(c.ac), CRIAR_CONTA);
    });
}

fn conversa(app: &mut WordApp, ui: &mut Ui, m: &mut Mentor, c: &Cores) {
    let rolar = std::mem::take(&mut m.rolar);
    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(Margin::symmetric(12, 12)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            if m.msgs.is_empty() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    estrela(ui, m, 44.0);
                    ui.label(RichText::new("Converse sobre o seu texto").font(semibold(18.0)).color(c.texto));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Selecione um trecho e toque em \"Usar a seleção\", ou faça uma pergunta geral. O Mentor lê o documento e responde com as técnicas de Protagonistas da Escrita.").font(regular(13.0)).color(c.fraco));
                });
            }
            let mut acao: Option<(u8, String)> = None;
            let n = m.msgs.len();
            for (i, msg) in m.msgs.iter().enumerate() {
                let largura = ui.available_width();
                let lay = if msg.eu { Layout::right_to_left(Align::Min) } else { Layout::left_to_right(Align::Min) };
                ui.with_layout(lay, |ui| {
                    let fill = if msg.eu { c.eu } else { c.bolha };
                    let canto = if msg.eu { CornerRadius { nw: 16, ne: 16, sw: 16, se: 5 } } else { CornerRadius { nw: 16, ne: 16, sw: 5, se: 16 } };
                    egui::Frame::NONE.fill(fill).stroke(Stroke::new(1.0, if msg.eu { Color32::TRANSPARENT } else { c.linha }))
                        .corner_radius(canto).inner_margin(Margin::symmetric(12, 9)).show(ui, |ui| {
                            ui.set_max_width(largura * if msg.eu { 0.88 } else { 1.0 } - 26.0);
                            ui.vertical(|ui| {
                                if !msg.trecho.is_empty() {
                                    let tr: String = msg.trecho.chars().take(160).collect();
                                    let rt = RichText::new(format!("“{}{}”", tr, if msg.trecho.chars().count() > 160 { "…" } else { "" })).italics().font(regular(12.5)).color(c.fraco);
                                    ui.add(egui::Label::new(rt).wrap());
                                }
                                if msg.erro {
                                    ui.add(egui::Label::new(RichText::new(&msg.texto).font(regular(13.5)).color(c.erro)).wrap());
                                } else if msg.eu {
                                    ui.add(egui::Label::new(RichText::new(&msg.texto).font(regular(13.5)).color(c.texto)).wrap());
                                } else if msg.texto.is_empty() {
                                    ui.label(RichText::new("O Mentor está lendo…").font(regular(13.0)).color(c.fraco));
                                } else {
                                    markdown(ui, &msg.texto, c);
                                    let terminou = i + 1 < n || m.tarefa != Tarefa::Respondendo;
                                    if terminou {
                                        ui.add_space(4.0);
                                        ui.horizontal_wrapped(|ui| {
                                            if botao(ui, c, "Inserir no texto", false).on_hover_text("Insere a resposta depois da seleção").clicked() {
                                                acao = Some((1, msg.texto.clone()));
                                            }
                                            if botao(ui, c, "Comentar na margem", false).on_hover_text("Selecione no documento o trecho que vai receber o comentário").clicked() {
                                                acao = Some((2, msg.texto.clone()));
                                            }
                                            if botao(ui, c, "Copiar", false).clicked() {
                                                acao = Some((3, msg.texto.clone()));
                                            }
                                        });
                                    }
                                }
                            });
                        });
                });
            }
            if let Some((tipo, texto)) = acao {
                let limpo = sem_markdown(&texto);
                let r = match tipo {
                    1 => inserir(app, &limpo).map(|_| "Inserido no texto."),
                    2 => comentar(app, &limpo).map(|_| "Comentário na margem."),
                    _ => {
                        ui.ctx().copy_text(limpo);
                        Ok("Copiado.")
                    }
                };
                match r {
                    Ok(t) => avisar(m, t),
                    Err(e) => avisar(m, &e),
                }
            }
            if rolar {
                ui.scroll_to_cursor(Some(Align::Max));
            }
        });
    });
    if let Some((t, quando)) = m.aviso.clone() {
        if now() - quando > 2.6 {
            m.aviso = None;
        } else {
            let r = ui.max_rect();
            let p = egui::pos2(r.center().x, r.bottom() - 18.0);
            let gal = ui.painter().layout_no_wrap(t, regular(12.5), c.fundo);
            let caixa = egui::Rect::from_center_size(p, gal.size() + vec2(24.0, 12.0));
            ui.painter().rect_filled(caixa, CornerRadius::same(14), c.texto);
            ui.painter().galley(caixa.center() - gal.size() / 2.0, gal, c.fundo);
        }
    }
}

fn compor(app: &mut WordApp, ui: &mut Ui, m: &mut Mentor, c: &Cores) {
    ui.spacing_mut().item_spacing = vec2(6.0, 7.0);
    if !m.trecho.is_empty() {
        egui::Frame::NONE.fill(c.ac_suave).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(9, 6)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let tr: String = m.trecho.chars().take(110).collect();
                ui.add(egui::Label::new(RichText::new(format!("“{tr}…”")).italics().font(regular(12.5)).color(c.fraco)).truncate());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new("×").font(regular(17.0)).color(c.fraco)).frame(false)).on_hover_text("Tirar o trecho").clicked() {
                        m.trecho.clear();
                    }
                });
            });
        });
    }
    ui.horizontal_wrapped(|ui| {
        if botao(ui, c, "Usar a seleção", false).clicked() {
            match app.execute("document.selection", json!({})) {
                Ok(v) => {
                    let t = v.get("text").and_then(Value::as_str).unwrap_or("").trim().to_string();
                    if t.is_empty() {
                        avisar(m, "Selecione um trecho no documento primeiro.");
                    } else {
                        m.trecho = t;
                    }
                }
                Err(e) => avisar(m, &e),
            }
        }
        ui.checkbox(&mut m.ler_doc, RichText::new("Ler o documento").font(regular(12.5)).color(c.texto));
        egui::ComboBox::from_id_salt("at_mentor_critica").selected_text(CRITICAS[m.critica].1).width(130.0).show_ui(ui, |ui| {
            for (i, (_, nome)) in CRITICAS.iter().enumerate() {
                ui.selectable_value(&mut m.critica, i, *nome);
            }
        });
    });
    let respondendo = m.tarefa == Tarefa::Respondendo;
    let mut mandar = false;
    egui::Frame::NONE.fill(c.bolha).stroke(Stroke::new(1.0, c.linha)).corner_radius(CornerRadius::same(18)).inner_margin(Margin { left: 10, right: 5, top: 5, bottom: 5 }).show(ui, |ui| {
        ui.horizontal(|ui| {
            let w = ui.available_width() - 48.0;
            let r = ui.add_sized(vec2(w, 34.0), egui::TextEdit::multiline(&mut m.pergunta).hint_text("Pergunte ao Mentor…").desired_rows(1).frame(egui::Frame::NONE).font(regular(13.5)));
            if r.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift) {
                while m.pergunta.ends_with('\n') {
                    m.pergunta.pop();
                }
                mandar = true;
            }
            if botao_enviar(ui, c, !respondendo).on_hover_text("Enviar (Enter)").clicked() {
                mandar = true;
            }
        });
    });
    if mandar && !respondendo && !m.pergunta.trim().is_empty() {
        perguntar(app, ui.ctx(), m);
    }
    ui.horizontal(|ui| {
        let enviando = m.tarefa == Tarefa::Enviando;
        let r = ui.add_enabled(!enviando, egui::Button::new(RichText::new(if enviando { "Enviando…" } else { "Enviar para o Autor Total online »" }).font(regular(12.0)).color(c.ac)).frame(false))
            .on_hover_text("Cria uma história nova na sua conta com o texto deste documento e abre o Autor Total no navegador, já na Escrita");
        if r.clicked() {
            enviar_online(app, ui.ctx(), m);
        }
    });
}

fn avisar(m: &mut Mentor, t: &str) {
    m.aviso = Some((t.to_string(), now()));
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn perguntar(app: &mut WordApp, ctx: &egui::Context, m: &mut Mentor) {
    let Some(s) = m.sessao.clone() else { return };
    let p = m.pergunta.trim().to_string();
    let trecho = std::mem::take(&mut m.trecho);
    let doc = if m.ler_doc && app.has_document() {
        app.execute("document.text", json!({})).ok().and_then(|v| v.get("text").and_then(Value::as_str).map(|t| t.chars().take(200_000).collect::<String>())).unwrap_or_default()
    } else {
        String::new()
    };
    let msg = if trecho.is_empty() { p.clone() } else { format!("Estou escrevendo um texto e quero conversar sobre ESTE TRECHO selecionado:\n\n«{trecho}»\n\nMinha pergunta: {p}") };
    m.msgs.push(Msg { eu: true, texto: p, trecho, erro: false });
    m.msgs.push(Msg { eu: false, texto: String::new(), trecho: String::new(), erro: false });
    m.pergunta.clear();
    m.tarefa = Tarefa::Respondendo;
    m.rolar = true;
    let hist: Vec<Value> = m.hist.iter().rev().take(8).rev().flat_map(|(q, r)| [json!({"role": "user", "content": q}), json!({"role": "assistant", "content": r})]).collect();
    m.hist.push((msg.chars().take(3000).collect(), String::new()));
    let critica = CRITICAS[m.critica].0.to_string();
    let (tx, rx) = channel();
    m.rx = Some(rx);
    trabalhar(ctx, tx, move |tx| {
        let s = match garantir(&s) {
            Ok(n) => {
                if n.access != s.access {
                    let _ = tx.send(Evento::Sessao(n.clone()));
                }
                n
            }
            Err(e) => {
                let _ = tx.send(Evento::Saiu(e));
                return;
            }
        };
        let corpo = json!({"question": msg, "history": hist, "conto": doc, "critica": critica, "origem": "escrita", "stream": true});
        if let Err(e) = perguntar_rede(&s, &corpo, tx) {
            let _ = tx.send(Evento::Erro(e));
        }
    });
}

fn enviar_online(app: &mut WordApp, ctx: &egui::Context, m: &mut Mentor) {
    let Some(s) = m.sessao.clone() else { return };
    if !app.has_document() {
        avisar(m, "Abra um documento primeiro.");
        return;
    }
    let texto = app.execute("document.text", json!({})).ok().and_then(|v| v.get("text").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
    if texto.trim().is_empty() {
        avisar(m, "O documento está vazio.");
        return;
    }
    let titulo = app.title_stem();
    m.tarefa = Tarefa::Enviando;
    let (tx, rx) = channel();
    m.rx = Some(rx);
    trabalhar(ctx, tx, move |tx| {
        let r = garantir(&s).and_then(|s| criar_historia(&s, &titulo, &texto));
        let _ = tx.send(match r {
            Ok(id) => Evento::Enviado(id),
            Err(e) => Evento::Erro(e),
        });
    });
}

fn receber(m: &mut Mentor) {
    let Some(rx) = m.rx.take() else { return };
    let mut manter = true;
    loop {
        match rx.try_recv() {
            Ok(ev) => match ev {
                Evento::Sessao(s) => {
                    guardar_refresh(&s.refresh);
                    if m.email.is_empty() {
                        m.email = s.email.clone();
                    }
                    m.sessao = Some(s);
                    m.pendente = None;
                    m.codigo.clear();
                    if m.tarefa == Tarefa::Entrando {
                        m.tarefa = Tarefa::Nada;
                        manter = false;
                    }
                }
                Evento::PrecisaCodigo(s, f) => {
                    m.pendente = Some((s, f));
                    m.tarefa = Tarefa::Nada;
                    manter = false;
                }
                Evento::Delta(d) => {
                    if let Some(u) = m.msgs.last_mut() {
                        u.texto.push_str(&d);
                    }
                }
                Evento::Fim => {
                    let resposta = m.msgs.last().map(|u| u.texto.clone()).unwrap_or_default();
                    if resposta.is_empty() {
                        if let Some(u) = m.msgs.last_mut() {
                            u.texto = "O Mentor não respondeu agora. Tente de novo.".into();
                            u.erro = true;
                        }
                        m.hist.pop();
                    } else if let Some(h) = m.hist.last_mut() {
                        h.1 = resposta.chars().take(3000).collect();
                    }
                    m.tarefa = Tarefa::Nada;
                    manter = false;
                }
                Evento::Erro(e) => {
                    match m.tarefa {
                        Tarefa::Entrando => m.erro_login = e,
                        Tarefa::Respondendo => {
                            m.hist.pop();
                            if let Some(u) = m.msgs.last_mut() {
                                u.texto = e;
                                u.erro = true;
                            }
                        }
                        _ => avisar(m, &e),
                    }
                    m.tarefa = Tarefa::Nada;
                    manter = false;
                }
                Evento::Enviado(id) => {
                    m.tarefa = Tarefa::Nada;
                    manter = false;
                    let url = format!("{SITE}?historia={id}");
                    if open_url(&url).is_err() {
                        avisar(m, "História criada. Abra o Autor Total online para ver.");
                    } else {
                        avisar(m, "História criada no Autor Total online.");
                    }
                }
                Evento::Saiu(e) => {
                    m.sessao = None;
                    m.pendente = None;
                    apagar_refresh();
                    m.erro_login = if e.is_empty() { String::new() } else { e };
                    if m.tarefa == Tarefa::Respondendo {
                        m.msgs.pop();
                        m.hist.pop();
                    }
                    m.tarefa = Tarefa::Nada;
                    manter = false;
                }
            },
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                if m.tarefa != Tarefa::Nada {
                    m.tarefa = Tarefa::Nada;
                }
                manter = false;
                break;
            }
        }
    }
    if manter {
        m.rx = Some(rx);
    }
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

// ---------- texto ----------

/// Markdown simples das respostas: parágrafos, listas, títulos e **negrito**.
fn markdown(ui: &mut Ui, texto: &str, c: &Cores) {
    for bloco in texto.split("\n\n") {
        let bloco = bloco.trim_matches('\n');
        if bloco.trim().is_empty() {
            continue;
        }
        let mut job = LayoutJob::default();
        for (n, linha) in bloco.lines().enumerate() {
            let mut l = linha.trim_end().to_string();
            let mut tam = 13.5;
            let mut forte = false;
            let mut citacao = false;
            if let Some(r) = l.trim_start().strip_prefix("### ").or_else(|| l.trim_start().strip_prefix("## ")).or_else(|| l.trim_start().strip_prefix("# ")) {
                l = r.to_string();
                tam = 15.0;
                forte = true;
            } else if let Some(r) = l.trim_start().strip_prefix("- ").or_else(|| l.trim_start().strip_prefix("* ")) {
                l = format!("•  {r}");
            } else if let Some(r) = l.trim_start().strip_prefix("> ") {
                l = r.to_string();
                citacao = true;
            }
            if n > 0 {
                job.append("\n", 0.0, TextFormat::simple(regular(tam), c.texto));
            }
            for (k, parte) in l.split("**").enumerate() {
                let negrito = forte || k % 2 == 1;
                let mut f = TextFormat::simple(if negrito { semibold(tam) } else { regular(tam) }, if citacao { c.fraco } else { c.texto });
                f.italics = citacao;
                job.append(parte, 0.0, f);
            }
        }
        job.wrap.max_width = ui.available_width();
        ui.add(egui::Label::new(job).wrap());
    }
}

fn sem_markdown(t: &str) -> String {
    t.lines()
        .map(|l| {
            let l = l.trim_start_matches("### ").trim_start_matches("## ").trim_start_matches("# ").trim_start_matches("> ");
            l.replace("**", "")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn inserir(app: &mut WordApp, texto: &str) -> Result<(), String> {
    if !app.has_document() {
        return Err("Abra um documento primeiro.".into());
    }
    let tinha = app.execute("document.selection", json!({}))?.get("text").and_then(Value::as_str).is_some_and(|t| !t.is_empty());
    app.execute("select.collapse", json!({"end": true}))?;
    // A seleção de um parágrafo inteiro termina no começo do próximo: volta para o fim do selecionado,
    // para a resposta não herdar o estilo (um título, por exemplo) nem colar no parágrafo seguinte.
    let agora = app.execute("document.selection", json!({}))?;
    if tinha && agora.get("focus").and_then(|f| f.get("off")).and_then(Value::as_u64) == Some(0) {
        let _ = app.execute("caret.left", json!({}));
    }
    let limpo = texto.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect::<Vec<_>>().join("\n");
    app.execute("text.insert", json!({"text": format!("\n{limpo}"), "paragraphs": true}))?;
    Ok(())
}

fn comentar(app: &mut WordApp, texto: &str) -> Result<(), String> {
    if !app.has_document() {
        return Err("Abra um documento primeiro.".into());
    }
    let sel = app.execute("document.selection", json!({}))?;
    if sel.get("text").and_then(Value::as_str).unwrap_or("").trim().is_empty() {
        return Err("Selecione no documento o trecho que vai receber o comentário.".into());
    }
    app.execute("review.newComment", json!({"text": texto}))?;
    Ok(())
}

// ---------- rede ----------

fn trabalhar(ctx: &egui::Context, tx: Sender<Evento>, f: impl FnOnce(&Sender<Evento>) + Send + 'static) {
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        f(&tx);
        ctx.request_repaint();
    });
}

fn agente(segundos: u64) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .https_only(true)
        .timeout_global(Some(Duration::from_secs(segundos)))
        .http_status_as_error(false)
        .user_agent("AutorTotalEscrita")
        .build()
        .new_agent()
}

fn ler_json(resp: &mut ureq::http::Response<ureq::Body>) -> Value {
    let mut s = String::new();
    let _ = resp.body_mut().with_config().reader().take(4_000_000).read_to_string(&mut s);
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

fn mensagem_erro(v: &Value, padrao: &str) -> String {
    for k in ["error_description", "msg", "message", "error"] {
        if let Some(s) = v.get(k).and_then(Value::as_str) {
            return traduzir(s);
        }
    }
    padrao.to_string()
}

fn traduzir(s: &str) -> String {
    let l = s.to_lowercase();
    if l.contains("invalid login credentials") {
        "E-mail ou senha incorretos.".into()
    } else if l.contains("email not confirmed") {
        "Confirme o seu e-mail antes de entrar (veja a sua caixa de entrada).".into()
    } else if l.contains("invalid totp") || l.contains("invalid code") {
        "Código incorreto. Confira o aplicativo autenticador e tente de novo.".into()
    } else {
        s.to_string()
    }
}

fn post(url: &str, token: Option<&str>, corpo: &Value, segundos: u64) -> Result<(u16, Value), String> {
    let mut req = agente(segundos).post(url).header("apikey", SB_KEY).header("Content-Type", "application/json");
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let mut resp = req.send(corpo.to_string()).map_err(|_| "Sem conexão com o Autor Total. Confira a internet.".to_string())?;
    let st = resp.status().as_u16();
    Ok((st, ler_json(&mut resp)))
}

fn sessao_de(v: &Value) -> Result<Sessao, String> {
    let access = v.get("access_token").and_then(Value::as_str).ok_or_else(|| mensagem_erro(v, "Não consegui entrar."))?.to_string();
    let user = v.get("user").cloned().unwrap_or(Value::Null);
    Ok(Sessao {
        access,
        refresh: v.get("refresh_token").and_then(Value::as_str).unwrap_or("").to_string(),
        email: user.get("email").and_then(Value::as_str).unwrap_or("").to_string(),
        user_id: user.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
        expira: v.get("expires_at").and_then(Value::as_i64).unwrap_or_else(|| now() as i64 + v.get("expires_in").and_then(Value::as_i64).unwrap_or(3600)),
    })
}

/// Entra com e-mail e senha. Devolve o fator TOTP quando a conta pede o código (verificação em duas etapas).
fn login(email: &str, senha: &str) -> Result<(Sessao, Option<String>), String> {
    let (st, v) = post(&format!("{SB_URL}/auth/v1/token?grant_type=password"), None, &json!({"email": email, "password": senha}), 25)?;
    if st >= 400 {
        return Err(mensagem_erro(&v, "Não consegui entrar."));
    }
    let s = sessao_de(&v)?;
    let fator = v.get("user").and_then(|u| u.get("factors")).and_then(Value::as_array).and_then(|fs| {
        fs.iter().find(|f| f.get("status").and_then(Value::as_str) == Some("verified") && f.get("factor_type").and_then(Value::as_str) == Some("totp"))
            .and_then(|f| f.get("id").and_then(Value::as_str).map(str::to_string))
    });
    Ok((s, fator))
}

fn verificar_codigo(s: &Sessao, fator: &str, codigo: &str) -> Result<Sessao, String> {
    let (st, v) = post(&format!("{SB_URL}/auth/v1/factors/{fator}/challenge"), Some(&s.access), &json!({}), 20)?;
    if st >= 400 {
        return Err(mensagem_erro(&v, "Não consegui pedir o código."));
    }
    let desafio = v.get("id").and_then(Value::as_str).ok_or("Não consegui pedir o código.")?.to_string();
    let (st, v) = post(&format!("{SB_URL}/auth/v1/factors/{fator}/verify"), Some(&s.access), &json!({"challenge_id": desafio, "code": codigo}), 20)?;
    if st >= 400 {
        return Err(mensagem_erro(&v, "Código incorreto."));
    }
    let mut n = sessao_de(&v)?;
    if n.email.is_empty() {
        n.email = s.email.clone();
    }
    if n.user_id.is_empty() {
        n.user_id = s.user_id.clone();
    }
    Ok(n)
}

fn renovar(refresh: &str) -> Result<Sessao, String> {
    let (st, v) = post(&format!("{SB_URL}/auth/v1/token?grant_type=refresh_token"), None, &json!({"refresh_token": refresh}), 20)?;
    if st >= 400 {
        return Err("Entre de novo para continuar.".into());
    }
    sessao_de(&v)
}

/// A sessão válida por mais um minuto pelo menos (renova se precisar).
fn garantir(s: &Sessao) -> Result<Sessao, String> {
    if s.expira > now() as i64 + 60 || s.refresh.is_empty() {
        return Ok(s.clone());
    }
    renovar(&s.refresh)
}

fn perguntar_rede(s: &Sessao, corpo: &Value, tx: &Sender<Evento>) -> Result<(), String> {
    let resp = agente(180)
        .post(&format!("{SB_URL}/functions/v1/scriba"))
        .header("apikey", SB_KEY)
        .header("Authorization", &format!("Bearer {}", s.access))
        .header("Content-Type", "application/json")
        .send(corpo.to_string());
    let mut resp = resp.map_err(|_| "Sem conexão com o Autor Total. Confira a internet.".to_string())?;
    let st = resp.status().as_u16();
    if st >= 400 {
        let v = ler_json(&mut resp);
        return Err(mensagem_erro(&v, &format!("O Mentor não respondeu (erro {st}).")));
    }
    let leitor = BufReader::new(resp.body_mut().with_config().limit(20_000_000).reader());
    for linha in leitor.lines() {
        let linha = linha.map_err(|_| "A conexão caiu no meio da resposta. Tente de novo.".to_string())?;
        let l = linha.trim();
        let Some(dado) = l.strip_prefix("data:") else { continue };
        let Ok(o) = serde_json::from_str::<Value>(dado.trim()) else { continue };
        if let Some(e) = o.get("erro").and_then(Value::as_str) {
            return Err(e.to_string());
        }
        if let Some(d) = o.get("delta").and_then(Value::as_str) {
            let _ = tx.send(Evento::Delta(d.to_string()));
        }
    }
    let _ = tx.send(Evento::Fim);
    Ok(())
}

fn html_de(texto: &str) -> String {
    texto
        .split('\n')
        .map(|p| {
            let e = p.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            if e.trim().is_empty() { "<p><br></p>".to_string() } else { format!("<p>{e}</p>") }
        })
        .collect()
}

/// Cria uma história nova na conta da pessoa com o texto do documento (na Escrita) e devolve o id.
fn criar_historia(s: &Sessao, titulo: &str, texto: &str) -> Result<String, String> {
    let corpo = json!({"user_id": s.user_id, "title": titulo.chars().take(120).collect::<String>(), "data": {"escrita": html_de(texto), "origem": "autor-total-escrita"}});
    let mut resp = agente(40)
        .post(&format!("{SB_URL}/rest/v1/projects?select=id"))
        .header("apikey", SB_KEY)
        .header("Authorization", &format!("Bearer {}", s.access))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=representation")
        .send(corpo.to_string())
        .map_err(|_| "Sem conexão com o Autor Total. Confira a internet.".to_string())?;
    let st = resp.status().as_u16();
    let v = ler_json(&mut resp);
    if st >= 400 {
        return Err(mensagem_erro(&v, &format!("Não consegui criar a história (erro {st}).")));
    }
    v.get(0).and_then(|o| o.get("id")).and_then(Value::as_str).map(str::to_string).ok_or_else(|| "Não consegui criar a história.".to_string())
}

// ---------- sessão guardada neste computador ----------

fn arquivo_sessao() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("AutorTotalEscrita").join("mentor-sessao"))
}

fn ler_refresh() -> Option<String> {
    let t = std::fs::read_to_string(arquivo_sessao()?).ok()?;
    let t = t.trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

fn guardar_refresh(r: &str) {
    if r.is_empty() {
        return;
    }
    if let Some(p) = arquivo_sessao() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, r);
    }
}

fn apagar_refresh() {
    if let Some(p) = arquivo_sessao() {
        let _ = std::fs::remove_file(p);
    }
}
