/**
 * Helper de autenticação Icube.
 * Use em qualquer página protegida.
 */
const icubeAuth = {
    currentUser: null,

    async check(redirectToLogin = true) {
        try {
            const r = await fetch('/api/me', { credentials: 'include' });
            const d = await r.json();
            if (!d.authenticated) {
                if (redirectToLogin) this.goToLogin();
                return null;
            }
            this.currentUser = d.user;
            return d.user;
        } catch (e) {
            console.error('[auth] erro ao verificar:', e);
            if (redirectToLogin) this.goToLogin();
            return null;
        }
    },
    goToLogin() {
        // O index.html é o login. Se já estiver numa página de login, não redireciona (evita loop)
        if (location.pathname === '/' || location.pathname.endsWith('/login.html')) return;
        const here = location.pathname + location.search;
        location.href = `/?redirect=${encodeURIComponent(here)}`;
    },

    async logout() {
        try {
            await fetch('/api/logout', { method: 'POST', credentials: 'include' });
        } catch (e) {
            console.error('[auth] erro no logout:', e);
        } finally {
            location.href = '/';
        }
    },
   
    handleAuthError(response) {
        if (response.status === 401) {
            this.goToLogin();
            return true;
        }
        return false;
    },




};