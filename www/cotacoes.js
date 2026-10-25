/* ===== Icube Cotações — com foto, link do fornecedor e EML ===== */
const API = '/api/equipamentos';
const PER_PAGE = 20;
const STATUS_OPTIONS = [
    { value:'pendente', label:'Pendente', cls:'status-avaliacao' },
    { value:'extraido', label:'Extraído', cls:'status-reparado'  },
    { value:'falhou',   label:'Falhou',   cls:'status-pecas'     },
    { value:'manual',   label:'Manual',   cls:'status-liberado'  },
];

const state = {
    page: 1, total: 0, rows: [], busy: false,
    filters: { peca:'', fornecedor:'', categoria:'', status:'' },
    selected: new Set(),
};

/* ===== Utils ===== */
function escapeIcube(s) {
    return String(s ?? '').replace(/\\/g,'\\\\').replace(/"/g,'\\"')
        .replace(/\n/g,'\\n').replace(/\r/g,'\\r').replace(/\t/g,'\\t');
}
function icubeStr(s)   { return `"${escapeIcube(s)}"`; }
function icubeInt(n)   { return String(Number(n) | 0); }
function icubeFloat(n) { return (n == null || isNaN(n)) ? 'null' : String(n); }

function escHtml(s) {
    return String(s ?? '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')
        .replace(/"/g,'&quot;').replace(/'/g,'&#39;');
}
function statusMeta(v) {
    return STATUS_OPTIONS.find(o => o.value === v) || { label: v, cls: '' };
}
function bestPrice(r) {
    const p = [r.preco_manual, r.preco_extraido, r.preco_lista].filter(x => x != null && !isNaN(x));
    return p.length ? Math.min(...p) : null;
}
function fmtPrice(v) {
    if (v == null) return '–';
    return 'R$ ' + Number(v).toFixed(2).replace('.', ',');
}
function hostFromUrl(url) {
    try {
        const u = new URL(url);
        return u.hostname.replace(/^www\./, '');
    } catch { return url || ''; }
}

/* ===== API ===== */
async function exec(code) {
    const ctrl = new AbortController();
    const to = setTimeout(() => ctrl.abort(), 20000);
    try {
        const resp = await fetch(API, {
            method: 'POST', credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ code }), signal: ctrl.signal,
        });
        if (!resp.ok) throw new Error('HTTP ' + resp.status);
        const data = await resp.json();
        if (!data.ok) throw new Error(data.error || 'Erro');
        return data.output;
    } catch (e) {
        if (e.name === 'AbortError') throw new Error('Timeout 20s');
        throw e;
    } finally { clearTimeout(to); }
}

function notify(msg, type = 'info', ms = 3500) {
    const el = document.getElementById('notification');
    if (!el) { console.log('[' + type + ']', msg); return; }
    el.textContent = msg;
    el.className = 'notification ' + type + ' show';
    clearTimeout(el._t);
    el._t = setTimeout(() => el.classList.remove('show'), ms);
}

function setBusy(b) {
    state.busy = b;
    document.querySelectorAll('.btn, .action-btn').forEach(x => x.disabled = b);
    updateExportButton();
}

/* ===== Stats ===== */
async function loadStats() {
    const g = document.getElementById('stats-grid');
    if (!g) return;
    try {
        const out = await exec(`
            db.connect_default();
            print(db.query_one("SELECT COUNT(*) AS n FROM cotacoes"));
            print(db.query("SELECT status_extracao, COUNT(*) AS n FROM cotacoes GROUP BY status_extracao"));
        `);
        const lines = out.trim().split('\n');
        const total = parseInt(lines[0] || '0', 10);
        const arr = JSON.parse(lines[1] || '[]');
        const c = { total };
        STATUS_OPTIONS.forEach(o => c[o.value] = 0);
        arr.forEach(r => { if (c[r.status_extracao] !== undefined) c[r.status_extracao] = Number(r.n) || 0; });
        g.innerHTML = `<div class="stat-card"><div class="stat-n">${c.total}</div><div class="stat-label">Total</div></div>` +
            STATUS_OPTIONS.map(o => `<div class="stat-card"><div class="stat-n">${c[o.value]||0}</div><div class="stat-label">${o.label}</div></div>`).join('');
    } catch (e) {
        g.innerHTML = `<div class="stat-card" style="grid-column:1/-1;background:#ffebee;color:#c62828;">
            <div class="stat-label">Erro: ${escHtml(e.message)}</div></div>`;
    }
}

/* ===== Filters ===== */
function buildWhere() {
    const f = state.filters;
    const conds = []; const params = []; let p = 1;
    if (f.peca)       { conds.push(`peca_nome ILIKE $${p++}`);       params.push(icubeStr(`%${f.peca}%`)); }
    if (f.fornecedor) { conds.push(`fornecedor_nome ILIKE $${p++}`); params.push(icubeStr(`%${f.fornecedor}%`)); }
    if (f.categoria)  { conds.push(`categoria ILIKE $${p++}`);       params.push(icubeStr(`%${f.categoria}%`)); }
    if (f.status)     { conds.push(`status_extracao = $${p++}`);     params.push(icubeStr(f.status)); }
    return { where: conds.length ? conds.join(' AND ') : '1=1', params };
}

function applyFilters() {
    const get = id => { const el = document.getElementById(id); return el ? el.value : ''; };
    state.filters = {
        peca:       get('f-peca').trim(),
        fornecedor: get('f-fornecedor').trim(),
        categoria:  get('f-categoria').trim(),
        status:     get('f-status'),
    };
    state.page = 1;
    loadRows();
}

function clearFilters() {
    ['f-peca','f-fornecedor','f-categoria','f-status']
        .forEach(id => { const el = document.getElementById(id); if (el) el.value = ''; });
    state.filters = { peca:'', fornecedor:'', categoria:'', status:'' };
    state.page = 1;
    loadRows();
}

/* ===== Rows ===== */
async function loadRows() {
    const c = document.getElementById('table-container');
    if (!c) return;
    c.innerHTML = '<div class="loading-state">Carregando...</div>';

    const { where, params } = buildWhere();
    const pStr = `[${params.join(',')}]`;
    const off = (state.page - 1) * PER_PAGE;
    const sqlList = `SELECT id, peca_nome, peca_modelo, fornecedor_nome, categoria, link, foto, preco_extraido, preco_lista, preco_manual, status_extracao, observacoes FROM cotacoes WHERE ${where} ORDER BY id DESC LIMIT ${PER_PAGE} OFFSET ${off}`;
    const sqlCount = `SELECT COUNT(*) AS n FROM cotacoes WHERE ${where}`;

    try {
        const out = await exec(`
            db.connect_default();
            print(db.query(${icubeStr(sqlList)}, ${pStr}));
            print(db.query_one(${icubeStr(sqlCount)}, ${pStr})["n"]);
        `);
        const lines = out.trim().split('\n');
        const rows = JSON.parse(lines[0] || '[]');
        state.total = parseInt(lines[1] || '0', 10);
        state.rows = rows;
        renderTable(rows);
        renderPagination();
        const info = document.getElementById('results-info');
        if (info) info.textContent = `${rows.length} de ${state.total} · Página ${state.page}`;
        updateExportButton();
    } catch (e) {
        c.innerHTML = `<div class="error-state"><strong>Erro:</strong><pre>${escHtml(e.message)}</pre></div>`;
    }
}

function renderTable(rows) {
    const c = document.getElementById('table-container');
    if (!rows.length) {
        c.innerHTML = '<div class="empty-state">Nenhuma cotação encontrada</div>';
        return;
    }

    const allVisible = rows.every(r => state.selected.has(r.id));
    const someSelected = rows.some(r => state.selected.has(r.id));

    let h = `<div class="table-wrapper"><table><thead><tr>
        <th class="checkbox-cell">
            <input type="checkbox" id="cb-all" ${allVisible ? 'checked' : ''}
                   onchange="toggleSelectAll(this.checked)">
        </th>
        <th>ID</th>
        <th>Foto</th>
        <th>Peça</th>
        <th>Fornecedor</th>
        <th>Link</th>
        <th>Melhor preço</th>
        <th>Status</th>
        <th>Ações</th>
        </tr></thead><tbody>`;

    for (const r of rows) {
        const m = statusMeta(r.status_extracao);
        const best = bestPrice(r);
        const checked = state.selected.has(r.id) ? 'checked' : '';

        const fotoCell = r.foto
            ? `<img src="${escHtml(r.foto)}" style="max-height:40px;max-width:60px;border-radius:4px;cursor:pointer;border:1px solid #e0e0e0;" onclick="previewPhoto('${escHtml(r.foto).replace(/'/g, "\\'")}')" title="Clique para ampliar">`
            : '<span style="color:#ccc;">–</span>';

        // Coluna Link: host clicável
        const linkCell = r.link
            ? `<a href="${escHtml(r.link)}" target="_blank" rel="noopener noreferrer"
                  style="color:#1976d2;text-decoration:none;font-weight:600;font-size:13px;"
                  title="${escHtml(r.link)}">
                   ${escHtml(hostFromUrl(r.link))} ↗
               </a>`
            : '<span style="color:#ccc;">–</span>';

        // Botão de abrir link (desabilitado se não tem link)
        const linkBtn = r.link
            ? `<button class="action-btn btn-link-external" title="Abrir link do fornecedor"
                       onclick="openSupplierLink('${escHtml(r.link).replace(/'/g, "\\'")}')">🔗</button>`
            : `<button class="action-btn btn-link-external" title="Sem link cadastrado" disabled
                       style="opacity:.3;cursor:not-allowed;">🔗</button>`;

        h += `<tr>
            <td class="checkbox-cell">
                <input type="checkbox" ${checked} onchange="toggleSelect(${Number(r.id)}, this.checked)">
            </td>
            <td>#${escHtml(r.id)}</td>
            <td>${fotoCell}</td>
            <td><strong>${escHtml(r.peca_nome)}</strong>${r.peca_modelo ? ' <small style="color:#666">(' + escHtml(r.peca_modelo) + ')</small>' : ''}</td>
            <td>${escHtml(r.fornecedor_nome || '–')}</td>
            <td>${linkCell}</td>
            <td class="price-best">${fmtPrice(best)}</td>
            <td><span class="status ${m.cls}">${escHtml(m.label)}</span></td>
            <td class="actions-cell">
                ${linkBtn}
                <button class="action-btn" title="Exportar EML" onclick="exportOneEml(${Number(r.id)})">📧</button>
                <button class="action-btn" title="Editar" onclick="openEditModal(${Number(r.id)})">✏️</button>
                <button class="action-btn" title="Remover" onclick="confirmDelete(${Number(r.id)})">🗑️</button>
                <button class="action-btn" title="IA" onclick="openAiModal(${Number(r.id)})">🤖</button>
            </td></tr>`;
    }
    c.innerHTML = h + '</tbody></table></div>';

    setTimeout(() => {
        const el = document.getElementById('cb-all');
        if (el) el.indeterminate = someSelected && !allVisible;
    }, 0);
}

/* ===== Abrir link do fornecedor ===== */
function openSupplierLink(url) {
    if (!url || url === '') {
        notify('Cotação sem link cadastrado', 'error');
        return;
    }
    window.open(url, '_blank', 'noopener,noreferrer');
}

function previewPhoto(url) {
    if (!url) return;
    openModal(`<div class="modal-overlay" onclick="if(event.target===this)closeAllModals()">
        <div class="modal" style="max-width:900px;background:transparent;box-shadow:none;">
            <div style="text-align:right;margin-bottom:8px;">
                <button class="modal-close" onclick="closeAllModals()"
                        style="background:white;font-size:32px;">×</button>
            </div>
            <img src="${escHtml(url)}" style="width:100%;height:auto;border-radius:8px;box-shadow:0 8px 24px rgba(0,0,0,.3);">
        </div>
    </div>`);
}

function renderPagination() {
    const el = document.getElementById('pagination');
    if (!el) return;
    const total = Math.max(1, Math.ceil(state.total / PER_PAGE));
    if (total <= 1) { el.innerHTML = ''; return; }
    let h = `<button onclick="goPage(1)" ${state.page===1?'disabled':''}>«</button>`;
    h += `<button onclick="goPage(${state.page-1})" ${state.page===1?'disabled':''}>‹</button>`;
    for (let i = Math.max(1,state.page-2); i <= Math.min(total,state.page+2); i++) {
        h += `<button class="${i===state.page?'active':''}" onclick="goPage(${i})">${i}</button>`;
    }
    h += `<button onclick="goPage(${state.page+1})" ${state.page===total?'disabled':''}>›</button>`;
    h += `<button onclick="goPage(${total})" ${state.page===total?'disabled':''}>»</button>`;
    el.innerHTML = h;
}

function goPage(n) {
    const total = Math.max(1, Math.ceil(state.total / PER_PAGE));
    if (n < 1 || n > total) return;
    state.page = n;
    loadRows();
}

/* ===== Seleção ===== */
function toggleSelect(id, checked) {
    if (checked) state.selected.add(id);
    else state.selected.delete(id);
    updateExportButton();
    const el = document.getElementById('cb-all');
    if (el) {
        const allVisible = state.rows.every(r => state.selected.has(r.id));
        const someSelected = state.rows.some(r => state.selected.has(r.id));
        el.checked = allVisible;
        el.indeterminate = someSelected && !allVisible;
    }
}

function toggleSelectAll(checked) {
    if (checked) state.rows.forEach(r => state.selected.add(r.id));
    else state.rows.forEach(r => state.selected.delete(r.id));
    renderTable(state.rows);
    updateExportButton();
}

function updateExportButton() {
    const btn = document.getElementById('btn-export-eml');
    if (!btn) return;
    const n = state.selected.size;
    btn.textContent = `📧 Exportar selecionados (${n})`;
    btn.disabled = n === 0 || state.busy;
    if (n > 0) btn.classList.add('ready');
    else btn.classList.remove('ready');
}

/* ===== Upload de foto ===== */
async function uploadPhoto() {
    const fileInput = document.getElementById('ff-foto-file');
    if (!fileInput || !fileInput.files[0]) return;

    const file = fileInput.files[0];
    if (file.size > 5 * 1024 * 1024) {
        notify('Foto muito grande (máx 5 MB)', 'error');
        fileInput.value = '';
        return;
    }

    const form = new FormData();
    form.append('file', file);

    const preview = document.getElementById('ff-foto-preview');
    if (preview) preview.innerHTML = '<span style="color:#666;font-size:13px;">Enviando...</span>';

    try {
        const r = await fetch('/api/upload', {
            method: 'POST',
            credentials: 'include',
            body: form,
        });

        const text = await r.text();
        let d;
        try {
            d = JSON.parse(text);
        } catch {
            throw new Error(`Servidor retornou ${r.status} ${r.statusText}: ${text.slice(0, 150) || '(vazio)'}`);
        }

        if (!d.ok) throw new Error(d.error || 'Erro no upload');

        document.getElementById('ff-foto').value = d.url;
        if (preview) {
            preview.innerHTML = `<img src="${d.url}" style="max-height:80px;border-radius:4px;">`;
        }
        const btnRem = document.getElementById('ff-foto-remove');
        if (btnRem) btnRem.disabled = false;
        notify('Foto enviada', 'success');
    } catch (e) {
        if (preview) preview.innerHTML = '';
        notify('Erro: ' + e.message, 'error', 7000);
    } finally {
        fileInput.value = '';
    }
}

function removePhoto() {
    document.getElementById('ff-foto').value = '';
    document.getElementById('ff-foto-preview').innerHTML = '';
    document.getElementById('ff-foto-file').value = '';
    document.getElementById('ff-foto-remove').disabled = true;
}

/* ============================================================
   EML — com foto embutida em base64
   ============================================================ */

function encodeRfc2047(str) {
    if (/^[\x00-\x7F]*$/.test(str)) return str;
    const utf8 = unescape(encodeURIComponent(str));
    return '=?UTF-8?B?' + btoa(utf8) + '?=';
}

function sanitizeFilename(s) {
    return String(s ?? 'cotacao')
        .normalize('NFD').replace(/[\u0300-\u036f]/g, '')
        .replace(/[^a-zA-Z0-9-_]/g, '-')
        .replace(/-+/g, '-').replace(/^-|-$/g, '')
        .toLowerCase().slice(0, 60);
}

const _imageCache = new Map();

/**
 * Converte URL de imagem em data URI base64 para embutir no EML.
 * Redimensiona para no máximo `maxDim` px no maior lado (padrão 480),
 * mantendo qualidade em telas retina (2x do tamanho exibido de 240px)
 * e reduzindo o tamanho do .eml de ~5MB para ~150KB por foto.
 */
async function imageToDataUri(url, maxDim = 480) {
    if (!url) return null;
    if (_imageCache.has(url)) return _imageCache.get(url);

    try {
        const resp = await fetch(url, { credentials: 'include' });
        if (!resp.ok) return null;
        const blob = await resp.blob();

        const dataUri = await new Promise((resolve) => {
            const img = new Image();
            img.crossOrigin = 'anonymous';
            img.onload = () => {
                const scale = Math.min(1, maxDim / Math.max(img.width, img.height));
                const w = Math.round(img.width * scale);
                const h = Math.round(img.height * scale);
                const canvas = document.createElement('canvas');
                canvas.width = w;
                canvas.height = h;
                const ctx = canvas.getContext('2d');
                ctx.drawImage(img, 0, 0, w, h);
                resolve(canvas.toDataURL('image/jpeg', 0.85));
            };
            img.onerror = () => resolve(null);
            img.src = URL.createObjectURL(blob);
        });

        if (dataUri) _imageCache.set(url, dataUri);
        return dataUri;
    } catch {
        return null;
    }
}

function buildEmlHtml(cotacoes, photosMap) {
    const dataGer = new Date().toLocaleString('pt-BR');

    let html = `<!DOCTYPE html>
<html lang="pt-BR">
<head><meta charset="UTF-8"></head>
<body style="font-family:Arial,Helvetica,sans-serif;color:#222;max-width:900px;margin:0 auto;padding:20px;background:#f5f5f5;">
    <div style="background:#fff;padding:24px;border-radius:8px;">
        <h2 style="color:#1976d2;border-bottom:2px solid #1976d2;padding-bottom:8px;margin-top:0;">
            Cotações — Icube
        </h2>
        <p style="color:#666;font-size:14px;">
            Gerado em ${escHtml(dataGer)} pelo sistema Icube.
        </p>

        <table border="0" cellpadding="12" cellspacing="0"
               style="border-collapse:collapse;width:100%;font-size:14px;border:1px solid #e0e0e0;">
            <thead>
                <tr style="background:#1976d2;color:#fff;">
                    <th align="left" style="padding:10px;border:1px solid #1976d2;width:260px;">Foto</th>
                    <th align="left" style="padding:10px;border:1px solid #1976d2;">Peça</th>
                    <th align="left" style="padding:10px;border:1px solid #1976d2;">Fornecedor</th>
                    <th align="left" style="padding:10px;border:1px solid #1976d2;">Observação</th>
                    <th align="left" style="padding:10px;border:1px solid #1976d2;">Link</th>
                </tr>
            </thead>
            <tbody>`;

    for (const c of cotacoes) {
        const peca = c.peca_nome || '–';
        const modelo = c.peca_modelo
            ? ` <span style="color:#666;font-weight:normal;">(${escHtml(c.peca_modelo)})</span>`
            : '';
        const obs = c.observacoes || '–';
        const fornecedor = c.fornecedor_nome || '–';
        const link = c.link || '';

        const linkHtml = link
            ? `<a href="${escHtml(link)}" style="color:#1976d2;text-decoration:none;">${escHtml(link)}</a>`
            : '<span style="color:#999;">–</span>';

        const dataUri = photosMap.get(c.id);

        // Foto 240px — envolta em div de 240px para máxima compatibilidade com Outlook
        const fotoHtml = dataUri
            ? `<div style="width:240px;max-width:100%;"><img src="${dataUri}" alt="Foto" width="240" style="display:block;width:100%;height:auto;border-radius:4px;border:1px solid #e0e0e0;"></div>`
            : (c.foto ? '<span style="color:#999;">(foto não disponível)</span>' : '<span style="color:#999;">–</span>');

        html += `
                <tr>
                    <td style="padding:10px;border:1px solid #e0e0e0;vertical-align:top;">${fotoHtml}</td>
                    <td style="padding:10px;border:1px solid #e0e0e0;vertical-align:top;"><strong>${escHtml(peca)}</strong>${modelo}</td>
                    <td style="padding:10px;border:1px solid #e0e0e0;vertical-align:top;">${escHtml(fornecedor)}</td>
                    <td style="padding:10px;border:1px solid #e0e0e0;vertical-align:top;">${escHtml(obs)}</td>
                    <td style="padding:10px;border:1px solid #e0e0e0;vertical-align:top;word-break:break-all;">${linkHtml}</td>
                </tr>`;
    }

    html += `
            </tbody>
        </table>
        <p style="color:#999;font-size:12px;margin-top:24px;text-align:center;">
            Este email foi gerado automaticamente pelo sistema Icube.
        </p>
    </div>
</body>
</html>`;
    return html;
}

function buildEml(cotacoes, photosMap, assunto) {
    const crlf = '\r\n';
    const now = new Date().toUTCString();
    const headers = [
        `From: Icube <noreply@icube.local>`,
        `To: destinatario@example.com`,
        `Subject: ${encodeRfc2047(assunto)}`,
        `Date: ${now}`,
        `MIME-Version: 1.0`,
        `Content-Type: text/html; charset=UTF-8`,
        `Content-Transfer-Encoding: 8bit`,
        `X-Mailer: Icube v0.7.5`,
    ];
    return headers.join(crlf) + crlf + crlf + buildEmlHtml(cotacoes, photosMap).replace(/\n/g, crlf);
}

function downloadEml(filename, content) {
    const blob = new Blob([content], { type: 'message/rfc822' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    setTimeout(() => URL.revokeObjectURL(url), 1000);
}

async function loadPhotosForEml(cotacoes) {
    const map = new Map();
    const tasks = cotacoes.map(async (c) => {
        if (!c.foto) { map.set(c.id, null); return; }
        const dataUri = await imageToDataUri(c.foto);
        map.set(c.id, dataUri);
    });
    await Promise.all(tasks);
    return map;
}

async function exportOneEml(id) {
    const row = state.rows.find(r => r.id === id);
    let cot = row;
    if (!cot || cot.observacoes === undefined || cot.foto === undefined) {
        setBusy(true);
        try { cot = await loadById(id); }
        catch (e) { notify('Erro: ' + e.message, 'error'); setBusy(false); return; }
        setBusy(false);
    }

    setBusy(true);
    notify('Preparando email...', 'info');
    try {
        const photosMap = await loadPhotosForEml([cot]);
        const assunto = `Cotação: ${cot.peca_nome}${cot.peca_modelo ? ' (' + cot.peca_modelo + ')' : ''}`;
        const eml = buildEml([cot], photosMap, assunto);
        const filename = `cotacao-${sanitizeFilename(cot.peca_nome)}-${id}.eml`;
        downloadEml(filename, eml);
        notify(`"${filename}" gerado`, 'success');
    } catch (e) {
        notify('Falha: ' + e.message, 'error', 5000);
    } finally { setBusy(false); }
}

async function exportSelectedEml() {
    const ids = Array.from(state.selected);
    if (ids.length === 0) { notify('Nenhuma cotação selecionada', 'error'); return; }

    setBusy(true);
    notify('Preparando email...', 'info');
    try {
        const idsStr = ids.join(',');
        const out = await exec(`
            db.connect_default();
            print(db.query("SELECT id, peca_nome, peca_modelo, fornecedor_nome, link, foto, observacoes FROM cotacoes WHERE id IN (${idsStr}) ORDER BY id ASC"));
        `);
        const cots = JSON.parse(out.trim());
        if (!cots.length) { notify('Cotações não encontradas', 'error'); return; }

        const photosMap = await loadPhotosForEml(cots);

        const assunto = `Cotações Icube — ${cots.length} item${cots.length > 1 ? 's' : ''}`;
        const eml = buildEml(cots, photosMap, assunto);
        const filename = `cotacoes-${cots.length}-${new Date().toISOString().slice(0,10)}.eml`;
        downloadEml(filename, eml);
        notify(`"${filename}" gerado (${cots.length} cotações)`, 'success');
    } catch (e) {
        notify('Falha: ' + e.message, 'error', 5000);
    } finally { setBusy(false); }
}

/* ===== Load by ID ===== */
async function loadById(id) {
    const out = await exec(`
        db.connect_default();
        let r = db.query_one("SELECT id, peca_nome, peca_modelo, fornecedor_nome, categoria, link, foto, preco_extraido, preco_lista, preco_manual, status_extracao, observacoes FROM cotacoes WHERE id = $1", [${icubeInt(id)}]);
        if r == null { throw "Não encontrada"; }
        print(r);
    `);
    return JSON.parse(out.trim());
}

/* ===== Modals ===== */
function openModal(h) {
    const root = document.getElementById('modal-root');
    if (!root) return;
    root.innerHTML = h;
    document.body.style.overflow = 'hidden';
}
function closeAllModals() {
    document.getElementById('modal-root').innerHTML = '';
    document.body.style.overflow = '';
}

function renderForm(d = {}) {
    const opts = STATUS_OPTIONS.map(o =>
        `<option value="${o.value}" ${d.status_extracao === o.value ? 'selected' : ''}>${o.label}</option>`
    ).join('');
    const pv = v => v == null ? '' : String(v);
    const fotoAtual = d.foto || '';

    return `<div class="filters-grid">
        <div class="field full-width"><label>Peça *</label>
            <input type="text" id="ff-peca" value="${escHtml(d.peca_nome||'')}" required></div>
        <div class="field"><label>Modelo</label>
            <input type="text" id="ff-modelo" value="${escHtml(d.peca_modelo||'')}"></div>
        <div class="field"><label>Categoria</label>
            <input type="text" id="ff-categoria" value="${escHtml(d.categoria||'')}"></div>
        <div class="field"><label>Fornecedor</label>
            <input type="text" id="ff-fornecedor" value="${escHtml(d.fornecedor_nome||'')}"></div>
        <div class="field"><label>Status *</label>
            <select id="ff-status">${opts}</select></div>
        <div class="field full-width"><label>Link *</label>
            <input type="url" id="ff-link" value="${escHtml(d.link||'')}" required></div>
        <div class="field"><label>Preço extraído</label>
            <input type="number" step="0.01" id="ff-px" value="${pv(d.preco_extraido)}"></div>
        <div class="field"><label>Preço lista</label>
            <input type="number" step="0.01" id="ff-pl" value="${pv(d.preco_lista)}"></div>
        <div class="field"><label>Preço manual</label>
            <input type="number" step="0.01" id="ff-pm" value="${pv(d.preco_manual)}"></div>
        <div class="field full-width"><label>Observações</label>
            <textarea id="ff-obs" rows="3">${escHtml(d.observacoes||'')}</textarea></div>

        <div class="field full-width">
            <label>Foto</label>
            <input type="hidden" id="ff-foto" value="${escHtml(fotoAtual)}">
            <div style="display:flex;gap:12px;align-items:flex-start;flex-wrap:wrap;">
                <div style="flex:1;min-width:200px;">
                    <input type="file" id="ff-foto-file" accept="image/jpeg,image/png,image/gif,image/webp"
                           onchange="uploadPhoto()"
                           style="padding:8px;border:1px solid #ddd;border-radius:4px;width:100%;background:#fafafa;">
                    <div style="font-size:11px;color:#999;margin-top:4px;">Formatos: jpg, png, gif, webp · máx 5 MB</div>
                </div>
                <div id="ff-foto-preview" style="flex-shrink:0;">
                    ${fotoAtual ? `<img src="${escHtml(fotoAtual)}" style="max-height:80px;border-radius:4px;border:1px solid #e0e0e0;">` : ''}
                </div>
                <button type="button" id="ff-foto-remove" class="btn btn-sm btn-secondary"
                        onclick="removePhoto()" ${fotoAtual ? '' : 'disabled'}>Remover</button>
            </div>
        </div>
    </div>`;
}

function collectForm() {
    const g = id => { const el = document.getElementById(id); return el ? el.value.trim() : ''; };
    const num = id => { const v = g(id); return v === '' ? null : parseFloat(v); };
    return {
        peca_nome: g('ff-peca'),
        peca_modelo: g('ff-modelo'),
        categoria: g('ff-categoria'),
        fornecedor_nome: g('ff-fornecedor'),
        status_extracao: g('ff-status'),
        link: g('ff-link'),
        preco_extraido: num('ff-px'),
        preco_lista: num('ff-pl'),
        preco_manual: num('ff-pm'),
        observacoes: g('ff-obs'),
        foto: g('ff-foto'),
    };
}

/* ===== CREATE ===== */
function openCreateModal() {
    openModal(`<div class="modal-overlay" onclick="if(event.target===this)closeAllModals()">
        <div class="modal"><div class="modal-header">
            <h2>➕ Nova Cotação</h2>
            <button class="modal-close" onclick="closeAllModals()">×</button>
        </div><div class="modal-body">
            <form onsubmit="submitCreate(event)">
                ${renderForm({ status_extracao: 'pendente' })}
                <div class="form-actions">
                    <button type="button" class="btn btn-secondary" onclick="closeAllModals()">Cancelar</button>
                    <button type="submit" class="btn btn-success">💾 Cadastrar</button>
                </div>
            </form>
        </div></div></div>`);
}

async function submitCreate(e) {
    e.preventDefault();
    const d = collectForm();
    if (!d.peca_nome || !d.link) { notify('Preencha peça e link', 'error'); return; }
    const cols = ['peca_nome','link','status_extracao'];
    const vals = [icubeStr(d.peca_nome), icubeStr(d.link), icubeStr(d.status_extracao)];
    if (d.peca_modelo)      { cols.push('peca_modelo');      vals.push(icubeStr(d.peca_modelo)); }
    if (d.categoria)        { cols.push('categoria');        vals.push(icubeStr(d.categoria)); }
    if (d.fornecedor_nome)  { cols.push('fornecedor_nome');  vals.push(icubeStr(d.fornecedor_nome)); }
    if (d.preco_extraido != null) { cols.push('preco_extraido'); vals.push(icubeFloat(d.preco_extraido)); }
    if (d.preco_lista != null)    { cols.push('preco_lista');    vals.push(icubeFloat(d.preco_lista)); }
    if (d.preco_manual != null)   { cols.push('preco_manual');   vals.push(icubeFloat(d.preco_manual)); }
    if (d.observacoes)      { cols.push('observacoes');      vals.push(icubeStr(d.observacoes)); }
    if (d.foto)             { cols.push('foto');             vals.push(icubeStr(d.foto)); }
    const ph = vals.map((_,i)=>'$'+(i+1)).join(',');
    const sql = `INSERT INTO cotacoes (${cols.join(',')}) VALUES (${ph})`;
    setBusy(true);
    try {
        await exec(`db.connect_default(); db.execute(${icubeStr(sql)}, [${vals.join(',')}]);`);
        closeAllModals();
        notify('Cadastrada', 'success');
        await Promise.all([loadStats(), loadRows()]);
    } catch (e) { notify('Falha: ' + e.message, 'error', 5000); }
    finally { setBusy(false); }
}

/* ===== EDIT ===== */
async function openEditModal(id) {
    setBusy(true);
    try {
        const d = await loadById(id);
        openModal(`<div class="modal-overlay" onclick="if(event.target===this)closeAllModals()">
            <div class="modal"><div class="modal-header">
                <h2>✏️ Editar #${id}</h2>
                <button class="modal-close" onclick="closeAllModals()">×</button>
            </div><div class="modal-body">
                <form onsubmit="submitEdit(event, ${id})">
                    ${renderForm(d)}
                    <div class="form-actions">
                        <button type="button" class="btn btn-secondary" onclick="closeAllModals()">Cancelar</button>
                        <button type="submit" class="btn btn-primary">💾 Salvar</button>
                    </div>
                </form>
            </div></div></div>`);
    } catch (e) { notify('Erro: ' + e.message, 'error', 5000); }
    finally { setBusy(false); }
}

async function submitEdit(e, id) {
    e.preventDefault();
    const d = collectForm();
    const sets = []; const vals = []; let p = 1;
    const push = (col, val) => {
        if (val === null || val === '') sets.push(`${col} = NULL`);
        else { sets.push(`${col} = $${p++}`); vals.push(val); }
    };
    sets.push(`peca_nome = $${p++}`); vals.push(icubeStr(d.peca_nome));
    sets.push(`link = $${p++}`); vals.push(icubeStr(d.link));
    sets.push(`status_extracao = $${p++}`); vals.push(icubeStr(d.status_extracao));
    push('peca_modelo', d.peca_modelo ? icubeStr(d.peca_modelo) : '');
    push('categoria', d.categoria ? icubeStr(d.categoria) : '');
    push('fornecedor_nome', d.fornecedor_nome ? icubeStr(d.fornecedor_nome) : '');
    push('preco_extraido', d.preco_extraido != null ? icubeFloat(d.preco_extraido) : '');
    push('preco_lista', d.preco_lista != null ? icubeFloat(d.preco_lista) : '');
    push('preco_manual', d.preco_manual != null ? icubeFloat(d.preco_manual) : '');
    push('observacoes', d.observacoes ? icubeStr(d.observacoes) : '');
    push('foto', d.foto ? icubeStr(d.foto) : '');
    sets.push('atualizado_em = now()');
    vals.push(icubeInt(id));
    const sql = `UPDATE cotacoes SET ${sets.join(',')} WHERE id = $${p}`;
    setBusy(true);
    try {
        const out = await exec(`db.connect_default(); print(db.execute(${icubeStr(sql)}, [${vals.join(',')}]));`);
        closeAllModals();
        if (parseInt(out.trim(),10) > 0) { notify('Atualizada', 'success'); await Promise.all([loadStats(), loadRows()]); }
        else notify('Nada alterado', 'error');
    } catch (e) { notify('Falha: ' + e.message, 'error', 5000); }
    finally { setBusy(false); }
}

/* ===== DELETE ===== */
function confirmDelete(id) {
    openModal(`<div class="modal-overlay" onclick="if(event.target===this)closeAllModals()">
        <div class="modal modal-sm"><div class="modal-header">
            <h2>🗑️ Remover #${id}</h2>
            <button class="modal-close" onclick="closeAllModals()">×</button>
        </div><div class="modal-body">
            <p>Confirma remoção da cotação <strong>#${id}</strong>?</p>
            <div class="form-actions">
                <button class="btn btn-secondary" onclick="closeAllModals()">Cancelar</button>
                <button class="btn btn-danger" onclick="submitDelete(${id})">Sim, remover</button>
            </div>
        </div></div></div>`);
}

async function submitDelete(id) {
    setBusy(true);
    try {
        const out = await exec(`db.connect_default(); print(db.execute("DELETE FROM cotacoes WHERE id = $1", [${icubeInt(id)}]));`);
        closeAllModals();
        if (parseInt(out.trim(),10) > 0) {
            state.selected.delete(id);
            notify('Removida', 'success');
            await Promise.all([loadStats(), loadRows()]);
        }
        else notify('Não encontrada', 'error');
    } catch (e) { notify('Falha: ' + e.message, 'error', 5000); }
    finally { setBusy(false); }
}

/* ===== AI ===== */
async function openAiModal(id) {
    setBusy(true);
    try {
        const d = await loadById(id);
        openModal(`<div class="modal-overlay" onclick="if(event.target===this)closeAllModals()">
            <div class="modal"><div class="modal-header">
                <h2>🤖 IA — #${id}</h2>
                <button class="modal-close" onclick="closeAllModals()">×</button>
            </div><div class="modal-body">
                <p><strong>${escHtml(d.peca_nome)}</strong></p>
                <div class="ai-buttons">
                    <button class="btn btn-primary" onclick="runAi('price',${id})">💰 Preços</button>
                    <button class="btn btn-primary" onclick="runAi('category',${id})">🏷️ Categoria</button>
                </div>
                <div id="ai-result"></div>
            </div></div></div>`);
    } catch (e) { notify('Erro: ' + e.message, 'error', 5000); }
    finally { setBusy(false); }
}

async function runAi(type, id) {
    const el = document.getElementById('ai-result');
    if (!el) return;
    el.innerHTML = '<div class="loading-state">Consultando IA...</div>';
    try {
        const d = await loadById(id);
        let prompt;
        if (type === 'price')
            prompt = 'Em português. Analise preços da peça "' + d.peca_nome + '": extraído R$ ' + (d.preco_extraido ?? 'n/a') + ', lista R$ ' + (d.preco_lista ?? 'n/a') + ', manual R$ ' + (d.preco_manual ?? 'n/a') + '. Qual o mais confiável? 2 frases.';
        else
            prompt = 'Em português, UMA palavra de categoria para a peça: "' + d.peca_nome + '".';
        const out = await exec(`
            if !ai.is_available() { throw "Ollama offline"; }
            print(ai.ask(${icubeStr(prompt)}));
        `);
        el.innerHTML = `<div class="ai-result">${escHtml(out.trim())}</div>`;
    } catch (e) {
        el.innerHTML = `<div class="error-state"><strong>Erro:</strong><pre>${escHtml(e.message)}</pre></div>`;
    }
}

/* ===== Logout ===== */
async function doLogout() {
    try { await fetch('/api/logout', { method:'POST', credentials:'include' }); } catch(e) {}
    location.href = '/';
}

/* ===== Boot ===== */
(async function boot() {
    console.log('[boot] iniciando');
    try {
        const me = await fetch('/api/me', { credentials: 'include' }).then(r => r.json());
        const badge = document.getElementById('user-badge');
        if (badge) {
            if (me && me.authenticated) {
                const rl = { administrator:'Admin', colaborador:'Colaborador', visitante:'Visitante' }[me.user.role] || me.user.role;
                badge.innerHTML = '👤 ' + escHtml(me.user.username) + ' <small>(' + escHtml(rl) + ')</small><button onclick="doLogout()">sair</button>';
            } else {
                badge.innerHTML = '⚠ sem auth <a href="/" style="color:#c62828">login</a>';
            }
        }
    } catch (e) {
        console.error('[boot] erro auth:', e);
    }
    await Promise.all([loadStats(), loadRows()]);
    console.log('[boot] concluído');
})();