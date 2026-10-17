// Hooks: one card per agent (Claude Code, Codex), each saying whether that agent is wired to Klat, with buttons to
// install, repair or remove its hooks. The user installs for one agent, the other, or both.
import type { HooksAgent, HooksInfo } from '../../bridge/tauriBridge';
import { banner, button, card, h } from '../controls';
import type { Ctx, SectionView } from '../context';
import { tMessage } from '../../i18n';
import { agentFolder, failureText, hooksSummary, hooksView } from '../viewModel';

export function hooksSection(ctx: Ctx): SectionView {
  const { t } = ctx;
  const latest = new Map<HooksAgent, HooksInfo>();
  const reportSummary = () => ctx.onHooksStatus(hooksSummary([...latest.values()].map((i) => i.status)));

  const agentCard = (first: HooksInfo): HTMLElement => {
    const { agent } = first;
    const name = t(`settings.hooks.agent.${agent}`);
    const body = h('div', { class: 'hooks-body', attrs: { 'aria-busy': 'false' } });
    const result = h('div', { class: 'hooks-result' });
    let busy = false;

    const render = (info: HooksInfo) => {
      const v = hooksView(info.status);
      body.setAttribute('aria-busy', String(busy));
      const run = (action: 'install' | 'uninstall') => () => {
        busy = true;
        result.replaceChildren();
        render(info);
        void ctx.api
          .hooksApply(agent, action)
          .then((m) => result.replaceChildren(banner('ok', tMessage(ctx.lang, m))))
          .catch((e: unknown) => result.replaceChildren(banner('error', failureText(ctx.lang, e, (reason) => reason))))
          .then(() => ctx.api.hooksStatus())
          .then((all) => {
            busy = false;
            for (const i of all) latest.set(i.agent, i);
            reportSummary();
            render(latest.get(agent) ?? info);
          });
      };
      const actions = h('div', { class: 'hooks-actions' });
      if (v.primary) {
        actions.append(button(busy ? t('settings.hooks.working') : t(v.primary.label), 'primary', `hooksPrimary-${agent}`, run('install')));
      }
      if (v.canUninstall) actions.append(button(t('settings.hooks.uninstall'), 'danger', `hooksUninstall-${agent}`, run('uninstall')));
      actions.querySelectorAll('button').forEach((b) => (b.disabled = busy));

      const missing =
        !info.agentFound && info.status === 'not-installed' && info.filePath
          ? h('p', { class: 'hooks-hint', text: t('settings.hooks.agentMissing', { agent: name, folder: agentFolder(info.filePath) }) })
          : null;
      const trust = agent === 'codex' ? h('p', { class: 'hooks-hint', text: t('settings.hooks.codexTrust') }) : null;

      body.replaceChildren(
        h(
          'div',
          { class: 'hooks-status' },
          h('span', { class: `pill pill-${v.tone}` }, h('span', { class: 'pill-dot', attrs: { 'aria-hidden': 'true' } }), t(v.label)),
          h('p', { class: 'hooks-desc', text: t(v.description, { agent: name }) }),
        ),
        missing ?? '',
        h(
          'div',
          { class: 'hooks-file' },
          h('div', { class: 'hooks-file-label', text: t('settings.hooks.file') }),
          h('code', { class: 'path', text: info.filePath ?? t('settings.hooks.fileUnknown') }),
        ),
        actions,
        trust ?? '',
      );
    };

    render(first);
    return card(name, h('div', { class: 'row row-block' }, body, result));
  };

  const cards = h('div', { class: 'hooks-agents', attrs: { 'aria-busy': 'true' } });
  void ctx.api.hooksStatus().then((all) => {
    for (const i of all) latest.set(i.agent, i);
    reportSummary();
    cards.setAttribute('aria-busy', 'false');
    cards.replaceChildren(...all.map(agentCard));
  });

  const el = h('div', {}, cards, h('p', { class: 'section-note', text: t('settings.hooks.note') }));
  return { el, update() {} };
}
