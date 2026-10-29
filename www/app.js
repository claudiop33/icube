/* ===== Icube Equipamentos ===== */

const API = '/api/equipamentos';
const PER_PAGE = 20;
const STATUS_OPTIONS = [
    { value: 'avaliacao', label: 'Avaliação', cls: 'status-avaliacao' },
    { value: 'pecas',     label: 'Peças',     cls: 'status-pecas'     },
    { value: 'reparado',  label: 'Reparado',  cls: 'status-reparado'  },
    { value: 'liberado',  label: 'Liberado',  cls: 'status-liberado'  },
];

const state = {
    page: 1, total: 0, rows: [], busy: false,
    filters: { letras:'', numeros:'', modelo:'', status:'', dataDe:'', dataAte:'' },
};

/* ===== Helpers ===== */
function escapeIcube(s) {
    return String(s ?? '').replace(/\\/g,'\\\\').replace(/"/g,'\\"')
        .replace(/\n/g,'\\n').replace(/\r/g,'\\r').replace(/\t/g,'\\t');
}
function icubeStr(s)  { return `"${escapeIcube(s)}"`; }
function icubeDate(s) { return s ? `${icubeStr(s)} as date` : 'null'; }
function icubeInt(n)  { return String(Number(n) | 0); }

function escHtml(s) {
    return String(s ?? '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')
        .replace(/"/g,'&quot;').replace(/'/g,'&#39;');
}

function statusMeta(v) {
    return STATUS_OPTIONS.find(o => o.value === v) || { label: v, cls: '' };
}

/* ===== API ===== */
async function exec(code) {
    const ctrl = new AbortController();
    const to = setTimeout(() => ctrl.abort(), 15000);
    try {
        const resp = await fetch(API, {
            method: 'POST',
            credentials: 'include',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ code }),
            signal: ctrl.signal,
        });
        if (resp.status === 401) { location.href = '/'; throw new Error('Não autenticado'); }
        if (!resp.ok) throw new Error('HTTP ' + resp.status);
        const data = await resp.json();
        if (!data.ok) throw new Error(data.error || 'Erro');
        return data.output;
    } catch (e) {
        if (e.name === 'AbortError') throw new Error('Timeout (15s) — servidor não respondeu');
        throw e;
    } finally {
        clearTimeout(to);
    }
}

/* ===== Notification ===== */
function notify(msg, type = 'info', ms = 3500) {
    const el = document.getElementById('notification');
    el.textContent = msg;
    el.className = 'notification ' + type + ' show';
    clearTimeout(el._t);
    el._t = setTimeout(() => el.classList.remove('show'), ms);
}

function setBusy(b) {
    state.busy = b;
    document.querySelectorAll('.btn, .action-btn').forEach(x => x.disabled = b);
}

/* ===== Health ===== */
async function checkHealth() {
    const el = document.getElementById('status-indicator');
    try {
        const r = await fetch('/health');
        el.textContent = r.ok ? '● Online' : '● Offline';
        el.className = 'status-badge ' + (r.ok ? 'status-online' : 'status-offline');
    } catch {
        el.textContent = '● Offline';
        el.className = 'status-badge status-offline';
    }
}

/* ===== Stats ===== */
async function loadStats() {
    const grid = document.getElementById('stats-grid');
    try {
        const out = await exec(`
            db.connect_default();
            let t = db.query_one("SELECT COUNT(*) AS n FROM equipamentos");
            print(t["n"]);
            let s = db.query("SELECT status, COUNT(*) AS n FROM equipamentos GROUP BY status");
            print(s);
        `);
        const lines = out.trim().split('\n');
        const total = parseInt(lines[0] || '0', 10);
        const arr = JSON.parse(lines[1] || '[]');
        const counts = { total };
        STATUS_OPTIONS.forEach(o => counts[o.value] = 0);
        arr.forEach(r => { if (counts[r.status] !== undefined) counts[r.status] = Number(r.n) || 0; });
        grid.innerHTML = `
            <div class="stat-card"><div class="stat-n">${counts.total}</div><div class="stat-label">Total</div></div>
            ${STATUS_OPTIONS.map(o =>
                `<div class="stat-card"><div class="stat-n">${counts[o.value] || 0}</div><div class="stat-label">${o.label}</div></div>`
            ).join('')}`;
    } catch (e) {
        grid.innerHTML = `<div class="stat-card" style="grid-column:1/-1;background:#ffebee;color:#c62828;">
            <div class="stat-label">Erro stats: ${escHtml(e.message)}</div></div>`;
    }
}

/* ===== Filters ===== */
function buildWhere() {
    const f = state.filters;
    const conds = []; const params = []; let p = 1;
    if (f.letras)  { conds.push(`codigo_letras ILIKE $${p++}`);  params.push(icubeStr(`%${f.letras}%`)); }
    if (f.numeros) { conds.push(`codigo_numeros ILIKE $${p++}`); params.push(icubeStr(`%${f.numeros}%`)); }
    if (f.modelo)  { conds.push(`modelo ILIKE $${p++}`);         params.push(icubeStr(`%${f.modelo}%`)); }
    if (f.status)  { conds.push(`status = $${p++}`);             params.push(icubeStr(f.status)); }
    if (f.dataDe)  { conds.push(`data_entrada >= $${p++}`);      params.push(icubeDate(f.dataDe)); }
    if (f.dataAte) { conds.push(`data_entrada <= $${p++}`);      params.push(icubeDate(f.dataAte)); }
    return { where: conds.length ? conds.join(' AND ') : '1=1', params };
}

function applyFilters() {
    state.filters = {
        letras: document.getElementById('f-letras').value.trim(),
        numeros: document.getElementById('f-numeros').value.trim(),
        modelo: document.getElementById('f-modelo').value.trim(),
        status: document.getElementById('f-status').value,
        dataDe: document.getElementById('f-data-de').value,
        dataAte: document.getElementById('f-data-ate').value,
    };
    state.page = 1;
    loadRows();
}

function clearFilters() {
    ['f-letras','f-numeros','f-modelo','f-status','f-data-de','f-data-ate']
        .forEach(id => document.getElementById(id).value = '');
    state.filters = { letras:'', numeros:'', modelo:'', status:'', dataDe:'', dataAte:'' };
    state.page = 1;
    loadRows();
}

function toggleFilters() {
    const b = document.getElementById('filters-body');
    const btn = document.getElementById('filters-toggle');
    if (b.style.display === 'none') { b.style.display = 'block'; btn.textContent = 'Recolher ▲'; }
    else { b.style.display = 'none'; btn.textContent = 'Expandir ▼'; }
}

/* ===== Rows ===== */
async function loadRows() {
    const c = document.getElementById('table-container');
    c.innerHTML = '<div class="loading-state">Carregando...</div>';
    const { where, params } = buildWhere();
    const offset = (state.page - 1) * PER_PAGE;
    const pStr = `[${params.join(', ')}]`;
    const sqlList = `SELECT id, codigo_letras, codigo_numeros, modelo, data_entrada, data_saida, defeito, status FROM equipamentos WHERE ${where} ORDER BY id DESC LIMIT ${PER_PAGE} OFFSET ${offset}`;
    const sqlCount = `SELECT COUNT(*) AS n FROM equipamentos WHERE ${where}`;
    try {
        const out = await exec(`
            db.connect_default();
            let r = db.query(${icubeStr(sqlList)}, ${pStr});
            let n = db.query_one(${icubeStr(sqlCount)}, ${pStr});
            print(r);
            print(n["n"]);
        `);
        const lines = out.trim().split('\n');
        const rows = JSON.parse(lines[0] || '[]');
        state.total = parseInt(lines[1] || '0', 10);
        state.rows = rows;
        renderTable(rows);
        renderPagination();
        document.getElementById('results-info').textContent =
            `Mostrando ${rows.length} de ${state.total} · Página ${state.page}`;
    } catch (e) {
        c.innerHTML = `<div class="error-state"><strong>Erro:</strong><pre>${escHtml(e.message)}</pre></div>`;
    }
}

function renderTable(rows) {
    const c = document.getElementById('table-container');
    if (!rows.length) { c.innerHTML = '<div class="empty-state">Nenhum equipamento encontrado</div>'; return; }
    let h = `<div class="table-wrapper"><table><thead><tr>
        <th>ID</th><th>Código</th><th>Modelo</th><th>Entrada</th><th>Status</th><th>Ações</th>
        </tr></thead><tbody>`;
    for (const r of rows) {
        const m = statusMeta(r.status);
        h += `<tr>
            <td>#${escHtml(r.id)}</td>
            <td><strong>${escHtml(r.codigo_letras)}-${escHtml(r.codigo_numeros)}</strong></td>
            <td>${escHtml(r.modelo || '–')}</td>
            <td>${escHtml(r.data_entrada || '–')}</td>
            <td><span class="status ${m.cls}">${escHtml(m.label)}</span></td>
            <td class="actions-cell">
                <button class="action-btn" onclick="openEditModal(${Number(r.id)})">✏️</button>
                <button class="action-btn" onclick="confirmDelete(${Number(r.id)})">🗑️</button>
                <button class="action-btn" onclick="openAiModal(${Number(r.id)})">🤖</button>
            </td></tr>`;
    }
    h += '</tbody></table></div>';
    c.innerHTML = h;
}

function renderPagination() {
    const el = document.getElementById('pagination');
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

/* ===== Load by ID ===== */
async function loadById(id) {
    const out = await exec(`
        db.connect_default();
        let r = db.query_one("SELECT id, codigo_letras, codigo_numeros, modelo, data_entrada, data_saida, defeito, relatorio_solucao, status, link_fornecedor, foto FROM equipamentos WHERE id = $1", [${icubeInt(id)}]);
        if r == null { throw "Não encontrado"; }
        print(r);
    `);
    return JSON.parse(out.trim());
}

/* ===== Modals ===== */
function openModal(html) {
    document.getElementById('modal-root').innerHTML = html;
    document.body.style.overflow = 'hidden';
}
function closeAllModals() {
    document.getElementById('modal-root').innerHTML = '';
    document.body.style.overflow = '';
}

function renderForm(d = {}) {
    const opts = STATUS_OPTIONS.map(o =>
        `<option value="${o.value}" ${d.status === o.value ? 'selected' : ''}>${o.label}</option>`
    ).join('');
    return `<div class="filters-grid">
        <div class="field"><label>Código letras *</label>
            <input type="text" id="ff-letras" value="${escHtml(d.codigo_letras||'')}" maxlength="20" required></div>
        <div class="field"><label>Código números *</label>
            <input type="text" id="ff-numeros" value="${escHtml(d.codigo_numeros||'')}" maxlength="20" required></div>
        <div class="field"><label>Modelo *</label>
            <input type="text" id="ff-modelo" value="${escHtml(d.modelo||'')}" maxlength="20" required></div>
        <div class="field"><label>Status *</label>
            <select id="ff-status">${opts}</select></div>
        <div class="field"><label>Data entrada *</label>
            <input type="date" id="ff-data-entrada" value="${escHtml(d.data_entrada||'')}" required></div>
        <div class="field"><label>Data saída</label>
            <input type="date" id="ff-data-saida" value="${escHtml(d.data_saida||'')}"></div>
        <div class="field full-width"><label>Defeito *</label>
            <textarea id="ff-defeito" rows="3" required>${escHtml(d.defeito||'')}</textarea></div>
        <div class="field full-width"><label>Relatório solução</label>
            <textarea id="ff-relatorio" rows="3">${escHtml(d.relatorio_solucao||'')}</textarea></div>
        <div class="field full-width"><label>Link fornecedor</label>
            <input type="url" id="ff-link" value="${escHtml(d.link_fornecedor||'')}"></div>
        <div class="field"><label>Foto</label>
            <input type="text" id="ff-foto" value="${escHtml(d.foto||'')}" maxlength="255"></div>
    </div>`;
}

function collectForm() {
    return {
        codigo_letras: document.getElementById('ff-letras').value.trim(),
        codigo_numeros: document.getElementById('ff-numeros').value.trim(),
        modelo: document.getElementById('ff-modelo').value.trim(),
        status: document.getElementById('ff-status').value,
        data_entrada: document.getElementById('ff-data-entrada').value,
        data_saida: document.getElementById('ff-data-saida').value,
        defeito: document.getElementById('ff-defeito').value.trim(),
        relatorio_solucao: document.getElementById('ff-relatorio').value.trim(),
        link_fornecedor: document.getElementById('ff-link').value.trim(),
        foto: document.getElementById('ff-foto').value.trim(),
    };
}

/* ===== CREATE ===== */
function openCreateModal() {
    const today = new Date().toISOString().slice(0, 10);
    openModal(`<div class="modal-overlay" onclick="if(event.target===this)closeAllModals()">
        <div class="modal"><div class="modal-header">
            <h2>➕ Novo Equipamento</h2>
            <button class="modal-close" onclick="closeAllModals()">×</button>
        </div><div class="modal-body">
            <form onsubmit="submitCreate(event)">
                ${renderForm({ data_entrada: today, status: 'avaliacao' })}
                <div class="form-actions">
                    <button type="button" class="btn btn-secondary" onclick="closeAllModals()">Cancelar</button>
                    <button type="submit" class="btn btn-success">💾 Cadastrar</button>
                </div>
            </form>
        </div></div></div>`);
    document.getElementById('ff-letras').focus();
}

async function submitCreate(e) {
    e.preventDefault();
    const d = collectForm();
    if (!d.codigo_letras || !d.codigo_numeros || !d.modelo || !d.data_entrada || !d.defeito) {
        notify('Preencha os campos obrigatórios', 'error'); return;
    }
    const cols = ['codigo_letras','codigo_numeros','modelo','data_entrada','defeito','status'];
    const vals = [icubeStr(d.codigo_letras), icubeStr(d.codigo_numeros), icubeStr(d.modelo),
                  icubeDate(d.data_entrada), icubeStr(d.defeito), icubeStr(d.status)];
    if (d.data_saida)        { cols.push('data_saida');        vals.push(icubeDate(d.data_saida)); }
    if (d.relatorio_solucao) { cols.push('relatorio_solucao'); vals.push(icubeStr(d.relatorio_solucao)); }
    if (d.link_fornecedor)   { cols.push('link_fornecedor');   vals.push(icubeStr(d.link_fornecedor)); }
    if (d.foto)              { cols.push('foto');              vals.push(icubeStr(d.foto)); }
    const ph = vals.map((_, i) => '$' + (i+1)).join(', ');
    const sql = `INSERT INTO equipamentos (${cols.join(',')}) VALUES (${ph})`;
    setBusy(true);
    try {
        await exec(`db.connect_default(); db.execute(${icubeStr(sql)}, [${vals.join(',')}]);`);
        closeAllModals();
        notify('Cadastrado com sucesso', 'success');
        await Promise.all([loadStats(), loadRows()]);
    } catch (e) {
        notify('Falha: ' + e.message, 'error', 5000);
    } finally { setBusy(false); }
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
    const push = (col, val, nullCol) => {
        if (val === null || val === '') sets.push(`${col} = NULL`);
        else { sets.push(`${col} = $${p++}`); vals.push(val); }
    };
    sets.push(`codigo_letras = $${p++}`); vals.push(icubeStr(d.codigo_letras));
    sets.push(`codigo_numeros = $${p++}`); vals.push(icubeStr(d.codigo_numeros));
    sets.push(`modelo = $${p++}`); vals.push(icubeStr(d.modelo));
    sets.push(`data_entrada = $${p++}`); vals.push(icubeDate(d.data_entrada));
    sets.push(`defeito = $${p++}`); vals.push(icubeStr(d.defeito));
    sets.push(`status = $${p++}`); vals.push(icubeStr(d.status));
    push('data_saida', d.data_saida ? icubeDate(d.data_saida) : '');
    push('relatorio_solucao', d.relatorio_solucao ? icubeStr(d.relatorio_solucao) : '');
    push('link_fornecedor', d.link_fornecedor ? icubeStr(d.link_fornecedor) : '');
    push('foto', d.foto ? icubeStr(d.foto) : '');
    vals.push(icubeInt(id));
    const sql = `UPDATE equipamentos SET ${sets.join(',')} WHERE id = $${p}`;
    setBusy(true);
    try {
        const out = await exec(`db.connect_default(); print(db.execute(${icubeStr(sql)}, [${vals.join(',')}]));`);
        const n = parseInt(out.trim(), 10);
        closeAllModals();
        if (n > 0) { notify('Atualizado', 'success'); await Promise.all([loadStats(), loadRows()]); }
        else notify('Nenhum registro alterado', 'error');
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
            <p>Confirma remoção do equipamento <strong>#${id}</strong>?</p>
            <div class="form-actions">
                <button class="btn btn-secondary" onclick="closeAllModals()">Cancelar</button>
                <button class="btn btn-danger" onclick="submitDelete(${id})">Sim, remover</button>
            </div>
        </div></div></div>`);
}

async function submitDelete(id) {
    setBusy(true);
    try {
        const out = await exec(`db.connect_default(); print(db.execute("DELETE FROM equipamentos WHERE id = $1", [${icubeInt(id)}]));`);
        const n = parseInt(out.trim(), 10);
        closeAllModals();
        if (n > 0) { notify('Removido', 'success'); await Promise.all([loadStats(), loadRows()]); }
        else notify('Não encontrado', 'error');
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
                <p><strong>${escHtml(d.codigo_letras)}-${escHtml(d.codigo_numeros)}</strong> · ${escHtml(d.modelo)}</p>
                <p style="color:#666;margin:8px 0;font-size:14px"><strong>Defeito:</strong> ${escHtml(d.defeito)}</p>
                <div class="ai-buttons">
                    <button class="btn btn-primary" onclick="runAi('urgency',${id})">🚨 Urgência</button>
                    <button class="btn btn-primary" onclick="runAi('solution',${id})">💡 Solução</button>
                    <button class="btn btn-primary" onclick="runAi('summary',${id})">📝 Resumo</button>
                </div>
                <div id="ai-result"></div>
            </div></div></div>`);
    } catch (e) { notify('Erro: ' + e.message, 'error', 5000); }
    finally { setBusy(false); }
}

async function runAi(type, id) {
    const el = document.getElementById('ai-result');
    el.innerHTML = '<div class="loading-state">Consultando IA...</div>';
    try {
        const d = await loadById(id);
        let prompt;
        if (type === 'urgency')
            prompt = 'Em português. Classifique a urgência em UMA palavra (critico, alto, medio, baixo): ' + d.defeito;
        else if (type === 'solution')
            prompt = 'Em português. Sugira solução em 2 frases. Modelo: ' + d.modelo + '\nDefeito: ' + d.defeito;
        else
            prompt = 'Em português. Resuma em 3 bullets:\n' + d.codigo_letras + '-' + d.codigo_numeros + '\nModelo: ' + d.modelo + '\nDefeito: ' + d.defeito + '\nStatus: ' + d.status;
        const out = await exec(`
            if !ai.is_available() { throw "Ollama offline"; }
            print(ai.ask(${icubeStr(prompt)}));
        `);
        el.innerHTML = `<div class="ai-result">${escHtml(out.trim())}</div>`;
    } catch (e) {
        el.innerHTML = `<div class="error-state"><strong>Erro IA:</strong><pre>${escHtml(e.message)}</pre></div>`;
    }
}

/* ===== Boot ===== */
(async function boot() {
    try {
        const me = await fetch('/api/me', { credentials: 'include' }).then(r => r.json());
        if (!me.authenticated) { location.href = '/'; return; }
        // user badge
        const badge = document.getElementById('user-badge');
        const roleLabel = { administrator:'Admin', colaborador:'Colaborador', visitante:'Visitante' }[me.user.role] || me.user.role;
        badge.innerHTML = '👤 ' + escHtml(me.user.username) +
            ' <small>(' + escHtml(roleLabel) + ')</small>' +
            '<button onclick="doLogout()">sair</button>';
        // run app
        checkHealth();
        await Promise.all([loadStats(), loadRows()]);
    } catch (e) {
        document.body.innerHTML = '<div style="padding:40px;text-align:center;color:#c62828">' +
            '<h1>Erro ao conectar</h1><pre>' + escHtml(String(e)) + '</pre>' +
            '<p><a href="/">← Voltar ao login</a></p></div>';
    }
})();

async function doLogout() {
    try { await fetch('/api/logout', { method: 'POST', credentials: 'include' }); } catch(e) {}
    location.href = '/';
}