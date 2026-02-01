1) Cause la plus probable (et je peux le prouver dans TON projet)

Dans ton ZIP, le dossier :

codex_home/

contient uniquement :

codex_home/config.toml

avec :

cli_auth_credentials_store = "file"


Or, d’après la doc officielle, quand cli_auth_credentials_store = "file", Codex stocke les tokens dans auth.json sous CODEX_HOME (qui vaut ~/.codex par défaut, mais toi tu forces CODEX_HOME=.../codex_home).

➡️ Donc si tu étais réellement loggé dans ce CODEX_HOME, tu devrais avoir :

codex_home/auth.json

… et il n’y est pas.

👉 Conclusion : tu n’es pas authentifié dans l’environnement que ton appli utilise, même si tu as peut‑être fait codex login dans ton terminal Windows “normal” (qui a écrit dans C:\Users\...\ .codex au lieu de codex_home).

C’est exactement le genre de situation qui finit en 401/403 (et Codex te l’affiche sous forme “unexpected status …”).

2) Vérif immédiate (dans ton appli, sans rien installer)
Étape A — vérifier le login Codex

Dans ton app tu as déjà Ctrl+T = codex login status.

La doc dit :

codex login status exit 0 quand tu es loggé.

➡️ Fais Ctrl+T et lis la sortie / le code de retour.

Étape B — login propre “portable”

Ensuite Ctrl+K (dans ton app) pour exécuter codex login.

La doc dit :

sans flag : ouvre un navigateur (OAuth ChatGPT)

--device-auth existe si le navigateur/callback localhost est bloqué

✅ Après login réussi :

tu dois voir apparaître codex_home/auth.json (vu que tu forces cli_auth_credentials_store="file").

⚠️ Sécurité : auth.json contient des tokens → c’est littéralement un “mot de passe”, ne le commit pas, ne le partage pas.

3) Autres causes possibles (très fréquentes) + comment les éliminer
A) Tu as une OPENAI_API_KEY (ou CODEX_API_KEY) “toxique” dans l’environnement

Si une clé API incorrecte traîne dans ton os.environ, Codex peut tenter de l’utiliser et te renvoyer du 401 Invalid Authentication.
401 = authentification invalide / mauvaise clé.

✅ Fix : dans ton appli, ne propage pas OPENAI_API_KEY / overrides d’endpoint par défaut (sauf si tu le veux explicitement).

B) Proxy / Firewall / réseau d’entreprise

Si tu es derrière un proxy, tu peux te prendre un status type 407 / 403 / etc.
Tu as déjà un preflight.py qui check DNS + proxy env vars → c’est très bien.

✅ Fix : si HTTP_PROXY/HTTPS_PROXY nécessaires, les définir correctement (ou whitelister les endpoints OpenAI).

C) Mauvaise méthode de login

La doc indique : “Codex cloud requires signing in with ChatGPT.”
Si tu as loggé avec “API key” mais que ton usage touche à Codex Cloud, tu peux te faire jeter.

✅ Fix : codex logout puis codex login en ChatGPT. (codex logout est dans la CLI).

4) Patch PRO dans ton projet : auto‑diagnostic + erreurs lisibles + env clean

Là ton UI affiche juste la ligne brute {"type":"error"...} et “rc=1”.
Tu veux que ton appli te dise clairement : “401 → pas loggé / token invalide → Ctrl+K” etc.

4.1 Patch usbide/app.py : nettoyer l’env Codex (évite les clés/URL parasites)

Dans ta classe USBIDEApp, ajoute ces helpers (importe re en haut du fichier) :

import re


Puis ajoute dans la classe :

    def _truthy(self, v: str | None) -> bool:
        return (v or "").strip().lower() in {"1", "true", "yes", "on"}

    def _sanitize_codex_env(self, env: dict[str, str]) -> dict[str, str]:
        """Empêche Codex de partir sur une auth/base URL involontaire.

        Par défaut on favorise login ChatGPT (tokens dans CODEX_HOME).
        On laisse l'utilisateur réactiver API key/custom base via flags USBIDE_*.
        """
        allow_api_key = self._truthy(os.environ.get("USBIDE_CODEX_ALLOW_API_KEY"))
        allow_custom_base = self._truthy(os.environ.get("USBIDE_CODEX_ALLOW_CUSTOM_BASE"))

        if not allow_api_key:
            env.pop("OPENAI_API_KEY", None)
            env.pop("CODEX_API_KEY", None)

        if not allow_custom_base:
            env.pop("OPENAI_BASE_URL", None)
            env.pop("OPENAI_API_BASE", None)
            env.pop("OPENAI_API_HOST", None)

        return env


Et modifie _codex_env() comme ça :

    def _codex_env(self) -> dict[str, str]:
        env = os.environ.copy()
        env.setdefault("PYTHONUTF8", "1")
        env.setdefault("PYTHONIOENCODING", "utf-8")
        env = self._portable_env(env)

        # IMPORTANT: évite que des variables globales cassent Codex
        env = self._sanitize_codex_env(env)

        return codex_env(self.root_dir, env)


🎯 Résultat : tu évites 80% des “unexpected status 401” causés par une clé env invalide.

4.2 Patch usbide/app.py : pré-check login status avant codex exec

Ajoute cette méthode :

    async def _codex_logged_in(self, env: dict[str, str]) -> bool:
        """Retourne True si codex login status = OK."""
        argv = codex_status_argv(self.root_dir, env)
        rc: int | None = None
        out_lines: list[str] = []

        async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
            if ev["kind"] == "line":
                out_lines.append(ev["text"])
            else:
                rc = ev["returncode"]

        if rc == 0:
            return True

        self._codex_log_ui("[yellow]Codex n'est pas authentifié dans ce CODEX_HOME.[/yellow]")
        for l in out_lines:
            if l.strip():
                self._codex_log_output(l)
        self._codex_log_ui("[yellow]Fais Ctrl+K pour `codex login` (ou device auth).[/yellow]")
        return False


Et dans _run_codex, juste après codex_cli_available(...) OK, mets :

        # Pré-check auth : évite des erreurs cryptiques "unexpected status"
        if not await self._codex_logged_in(env):
            return

4.3 Patch usbide/app.py : parser les erreurs JSON et afficher un diagnostic (401/403/407…)

Ajoute ces helpers :

    def _extract_status_code(self, msg: str) -> int | None:
        # Exemples vus en pratique:
        # "unexpected status 401 Unauthorized: ..."
        # "exceeded retry limit, last status: 401 Unauthorized"
        m = re.search(r"(?:unexpected status|last status[: ]+)\s*(\d{3})", msg, flags=re.IGNORECASE)
        if not m:
            m = re.search(r"\b(\d{3})\b", msg)
        if not m:
            return None
        try:
            return int(m.group(1))
        except Exception:
            return None

    def _codex_hint_for_status(self, status: int) -> str | None:
        if status == 401:
            return "401 = authentification invalide → Ctrl+K (login) ou `codex logout` + login ChatGPT."
        if status == 403:
            return "403 = accès interdit → vérifie login ChatGPT (pas API key) / droits workspace / réseau entreprise."
        if status == 407:
            return "407 = proxy auth required → configure HTTP_PROXY/HTTPS_PROXY."
        if status == 429:
            return "429 = rate limit → réessaie plus tard / ralentis."
        if 500 <= status <= 599:
            return "5xx = erreur serveur → réessaie, possible incident côté OpenAI."
        return None


Puis remplace le bloc d’affichage JSON dans _run_codex par une version qui extrait le message :

                try:
                    obj = json.loads(line)
                except Exception:
                    self._codex_log_output(line)
                    continue

                t = obj.get("type") if isinstance(obj, dict) else None

                # Affiche les erreurs de manière lisible
                if t == "error" and isinstance(obj, dict):
                    msg = str(obj.get("message", ""))
                    status = self._extract_status_code(msg) if msg else None
                    if status:
                        self._codex_log_ui(f"[red]Erreur Codex HTTP {status}[/red] {rich_escape(msg)}")
                        hint = self._codex_hint_for_status(status)
                        if hint:
                            self._codex_log_ui(f"[yellow]{rich_escape(hint)}[/yellow]")
                    else:
                        self._codex_log_ui(f"[red]Erreur Codex[/red] {rich_escape(msg)}")
                    continue

                if t == "turn.failed" and isinstance(obj, dict):
                    err = obj.get("error")
                    msg = ""
                    if isinstance(err, dict):
                        msg = str(err.get("message", "")) or str(err)
                    else:
                        msg = str(err)
                    status = self._extract_status_code(msg) if msg else None
                    if status:
                        self._codex_log_ui(f"[red]Task échouée HTTP {status}[/red] {rich_escape(msg)}")
                        hint = self._codex_hint_for_status(status)
                        if hint:
                            self._codex_log_ui(f"[yellow]{rich_escape(hint)}[/yellow]")
                    else:
                        self._codex_log_ui(f"[red]Task échouée[/red] {rich_escape(msg)}")
                    continue

                # Sinon: log brut (ou enrichi)
                if isinstance(obj, dict) and isinstance(obj.get("type"), str):
                    self._codex_log_output(f"[{obj.get('type')}] {json.dumps(obj, ensure_ascii=False)}")
                else:
                    self._codex_log_output(json.dumps(obj, ensure_ascii=False))


🎯 Résultat : au lieu d’un vague “unexpected status”, ton IDE affichera :

“HTTP 401 → login”

ou “HTTP 407 → proxy”

etc.

5) Option très utile : forcer “login ChatGPT only” dans codex_home/config.toml

La doc montre une option forced_login_method qui peut forcer un type de login.

Dans ton codex_home/config.toml, tu peux mettre :

cli_auth_credentials_store = "file"
forced_login_method = "chatgpt"


Ça évite que Codex passe en mode “API key” si une variable d’environnement se balade.

6) Si malgré ça tu as encore l’erreur : procédure “reset clean” (ultra efficace)

Dans ton panneau “Commande” (ou terminal), avec ton CODEX_HOME de la clé :

logout

codex logout


(codex logout existe officiellement)

supprime les tokens file (si tu utilises cli_auth_credentials_store="file")

del codex_home\auth.json


relog

codex login


ou device auth :

codex login --device-auth


TL;DR (ce qui casse chez toi)

Ton appli force CODEX_HOME=.../codex_home

mais tu n’as pas de codex_home/auth.json

donc Codex n’est pas loggé dans cet environnement

et il part faire une requête → backend répond non (401/403/…) → unexpected status → turn.failed

Applique :

Ctrl+K login (ou --device-auth)

patch env clean + pré-check login + parser erreurs (sections 4.x)