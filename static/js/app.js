//  theme 
(function () {
  const saved = localStorage.getItem('claudia_theme') || 'dark';
  applyTheme(saved);
})();

function applyTheme(t) {
  document.documentElement.setAttribute('data-theme', t);
  document.getElementById('hljs-theme-dark').disabled  = (t === 'light');
  document.getElementById('hljs-theme-light').disabled = (t === 'dark');
  const sun  = document.getElementById('icon-sun');
  const moon = document.getElementById('icon-moon');
  if (sun && moon) {
    sun.style.display  = t === 'dark'  ? '' : 'none';
    moon.style.display = t === 'light' ? '' : 'none';
  }
  localStorage.setItem('claudia_theme', t);
}

function toggleTheme() {
  const current = document.documentElement.getAttribute('data-theme');
  applyTheme(current === 'dark' ? 'light' : 'dark');
}

//  session ID 
function getSessionId() {
  let sid = sessionStorage.getItem('claudia_session');
  if (!sid) { sid = crypto.randomUUID(); sessionStorage.setItem('claudia_session', sid); }
  return sid;
}

//  sidebar 
let sidebarOpen = true;

function toggleSidebar() {
  sidebarOpen = !sidebarOpen;
  document.getElementById('sidebar').classList.toggle('collapsed', !sidebarOpen);
  if (sidebarOpen) loadSessions();
}

async function loadSessions() {
  const list = document.getElementById('session-list');
  try {
    const r = await fetch(`${BASE}/api/sessions`);
    if (r.status === 401) { window.location = `${BASE}/login`; return; }
    if (!r.ok) { list.innerHTML = '<div class="sidebar-empty">Failed to load</div>'; return; }
    const sessions = await r.json();
    if (!sessions.length) {
      list.innerHTML = '<div class="sidebar-empty">No history yet</div>';
      return;
    }
    list.innerHTML = sessions.map(s => `
      <div class="session-item" onclick="openSession('${s.session_id}')" data-sid="${s.session_id}">
        <div class="s-title">${escHtml(truncate(s.first_message, 38))}</div>
        <div class="s-meta">${s.turn_count} turn${s.turn_count !== 1 ? 's' : ''} · ${fmtDate(s.started_at)}</div>
      </div>`).join('');
  } catch(e) {
    list.innerHTML = '<div class="sidebar-empty">Failed to load</div>';
  }
}

async function openSession(sid) {
  // highlight active
  document.querySelectorAll('.session-item').forEach(el =>
    el.classList.toggle('active', el.dataset.sid === sid));

  const panel = document.getElementById('history-panel');
  const msgs  = document.getElementById('messages');
  panel.innerHTML = '';
  panel.classList.remove('visible');

  try {
    const r    = await fetch(`${BASE}/api/sessions/${sid}`);
    if (r.status === 401) { window.location = `${BASE}/login`; return; }
    if (!r.ok) throw new Error(`server returned ${r.status}`);
    const turns = await r.json();

    const btnRow = document.createElement('div');
    btnRow.style.cssText = 'display:flex;gap:.5rem;margin-bottom:.5rem';

    const back = document.createElement('button');
    back.className = 'h-back-btn';
    back.textContent = '← Back';
    back.onclick = closeHistoryPanel;
    btnRow.appendChild(back);

    const resume = document.createElement('button');
    resume.className = 'h-back-btn';
    resume.textContent = '▶ Resume';
    resume.style.cssText = 'background:var(--accent);color:#fff;border-color:var(--accent)';
    resume.onclick = () => resumeSession(sid, turns);
    btnRow.appendChild(resume);

    panel.appendChild(btnRow);

    const title = document.createElement('h2');
    title.textContent = `Session — ${turns.length} turn${turns.length !== 1 ? 's' : ''}`;
    panel.appendChild(title);

    turns.forEach(t => {
      const div = document.createElement('div');
      div.className = 'h-turn';
      div.innerHTML = `
        <div class="h-meta">${fmtDate(t.created_at)}</div>
        <div class="h-user">${escHtml(t.user_message)}</div>
        <div class="h-assistant content">${renderMarkdown(t.assistant_response)}</div>`;
      panel.appendChild(div);
      if (window.hljs) div.querySelectorAll('pre code').forEach(hljs.highlightElement);
    });

    msgs.style.display     = 'none';
    panel.classList.add('visible');
  } catch(e) {
    panel.innerHTML = `<p style="color:var(--danger)">Failed to load session: ${e.message}</p>`;
    panel.classList.add('visible');
  }
}

function closeHistoryPanel() {
  const panel = document.getElementById('history-panel');
  panel.classList.remove('visible');
  panel.innerHTML = '';
  document.getElementById('messages').style.display = '';
  document.querySelectorAll('.session-item').forEach(el => el.classList.remove('active'));
}

function resumeSession(sid, turns) {
  // restore session id so next message continues this session
  sessionStorage.setItem('claudia_session', sid);

  // rebuild history array for the API
  const history = [];
  turns.forEach(t => {
    history.push({ role: 'user',      content: t.user_message });
    history.push({ role: 'assistant', content: t.assistant_response });
  });
  document.getElementById('cl-history').value = JSON.stringify(history);

  // render all turns into the chat view
  closeHistoryPanel();
  const msgs = document.getElementById('messages');
  msgs.innerHTML = '';
  turns.forEach(t => {
    const userBubble = document.createElement('div');
    userBubble.className = 'message user';
    userBubble.innerHTML = `
      <div class="avatar user">You</div>
      <div class="content"><p>${escHtml(t.user_message).replace(/\n/g,'<br>')}</p></div>`;
    msgs.appendChild(userBubble);

    const aiBubble = document.createElement('div');
    aiBubble.className = 'message assistant';
    aiBubble.innerHTML = `
      <div class="avatar assistant">AI</div>
      <div class="content">${renderMarkdown(t.assistant_response)}</div>`;
    msgs.appendChild(aiBubble);
    if (window.hljs) aiBubble.querySelectorAll('pre code').forEach(hljs.highlightElement);
  });
  scrollToBottom();
  document.getElementById('message').focus();
}

//  helpers 
function escHtml(s) {
  return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
}
function truncate(s, n) { return s.length > n ? s.slice(0, n) + '…' : s; }
function fmtDate(iso) {
  try { return new Date(iso).toLocaleString(undefined, { month:'short', day:'numeric', hour:'2-digit', minute:'2-digit' }); }
  catch { return iso; }
}
function renderMarkdown(text) {
  if (window.marked) return marked.parse(text);
  return '<p>' + escHtml(text).replace(/\n/g,'<br>') + '</p>';
}
function autoResize(ta) { ta.style.height = 'auto'; ta.style.height = ta.scrollHeight + 'px'; }
function scrollToBottom() {
  const sa = document.getElementById('scroll-area');
  sa.scrollTop = sa.scrollHeight;
}
function handleKey(e) {
  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); document.getElementById('chat-form').requestSubmit(); }
}

function newChat() {
  sessionStorage.removeItem('claudia_session');
  document.getElementById('cl-history').value = '[]';
  closeHistoryPanel();
  document.getElementById('messages').innerHTML = `
    <div id="empty-state">
      <svg viewBox="0 0 24 24" fill="currentColor" style="width:44px;height:44px;opacity:.3">
        <path d="M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2z"/>
      </svg>
      <p>How can I help you today?</p>
    </div>`;
  if (sidebarOpen) loadSessions();
}

//  chat 
async function sendChat(e) {
  e.preventDefault();

  // if history panel is showing, go back to chat first
  closeHistoryPanel();

  const textarea = document.getElementById('message');
  const msg = textarea.value.trim();
  if (!msg) return;

  const history = JSON.parse(document.getElementById('cl-history').value || '[]');
  const msgs    = document.getElementById('messages');

  // remove empty state
  document.getElementById('empty-state')?.remove();

  // user bubble
  const userBubble = document.createElement('div');
  userBubble.className = 'message user';
  userBubble.innerHTML = `
    <div class="avatar user">You</div>
    <div class="content"><p>${escHtml(msg).replace(/\n/g,'<br>')}</p></div>`;
  msgs.appendChild(userBubble);

  // thinking indicator
  const thinking = document.createElement('div');
  thinking.className = 'thinking';
  thinking.id = 'thinking-indicator';
  thinking.innerHTML = `
    <div class="avatar assistant">AI</div>
    <div class="dot-pulse"><span></span><span></span><span></span></div>`;
  msgs.appendChild(thinking);
  scrollToBottom();

  textarea.value = '';
  textarea.style.height = 'auto';
  const sendBtn = document.getElementById('send-btn');
  sendBtn.disabled = true;

  try {
    const resp = await fetch(`${BASE}/chat`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-HTTP-Method-Override': 'GET',
      },
      body: JSON.stringify({ message: msg, history, session_id: getSessionId() }),
    });

    document.getElementById('thinking-indicator')?.remove();

    if (!resp.ok) {
      const errText = await resp.text();
      const errEl = document.createElement('div');
      errEl.className = 'error-msg';
      errEl.textContent = `Error: ${errText}`;
      msgs.appendChild(errEl);
      scrollToBottom();
      return;
    }

    const html = await resp.text();
    const tmp  = document.createElement('div');
    tmp.innerHTML = html;

    // extract updated history
    const historyEl = tmp.querySelector('[data-history-update]');
    if (historyEl) {
      document.getElementById('cl-history').value = historyEl.getAttribute('data-history-update');
      historyEl.remove();
    }

    // render markdown in assistant content
    tmp.querySelectorAll('.assistant-raw-content').forEach(el => {
      el.innerHTML = renderMarkdown(el.textContent);
      el.classList.remove('assistant-raw-content');
      if (window.hljs) el.querySelectorAll('pre code').forEach(hljs.highlightElement);
    });

    msgs.appendChild(tmp);
    scrollToBottom();

    // refresh sidebar if open
    if (sidebarOpen) loadSessions();

  } catch(err) {
    document.getElementById('thinking-indicator')?.remove();
    const errEl = document.createElement('div');
    errEl.className = 'error-msg';
    errEl.textContent = `Network error: ${err.message}`;
    msgs.appendChild(errEl);
    scrollToBottom();
  } finally {
    sendBtn.disabled = false;
    textarea.focus();
  }
}

// init: load sessions on page load
loadSessions();
