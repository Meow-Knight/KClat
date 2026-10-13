// Hooks: whether Claude Code is wired to Klat, and buttons to install, repair or remove the hooks.
import type { HooksInfo } from '../../bridge/tauriBridge';
import { banner, button, card, h } from '../controls';
import type { Ctx, SectionView } from '../context';
import { hooksView } from '../viewModel';

export function hooksSection(ctx: Ctx): SectionView {
  const { t } = ctx;
  const body = h('div', { class: 'hooks-body', attrs: { 'aria-busy': 'true' } });
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
        .hooksApply(action)
        .then((r) => result.replaceChildren(banner(r.ok ? 'ok' : 'error', r.message)))
        .catch((e: unknown) => result.replaceChildren(banner('error', String(e))))
        .then(() => ctx.api.hooksStatus())
        .then((next) => {
          busy = false;
          ctx.onHooksStatus(next.status);
          render(next);
        });
    };
    const actions = h('div', { class: 'hooks-actions' });
    if (v.primary) actions.append(button(busy ? t('settings.hooks.working') : t(v.primary.label), 'primary', 'hooksPrimary', run('install')));
    if (v.canUninstall) actions.append(button(t('settings.hooks.uninstall'), 'danger', 'hooksUninstall', run('uninstall')));
    actions.querySelectorAll('button').forEach((b) => (b.disabled = busy));

    body.replaceChildren(
      h(
        'div',
        { class: 'hooks-status' },
        h('span', { class: `pill pill-${v.tone}` }, h('span', { class: 'pill-dot', attrs: { 'aria-hidden': 'true' } }), t(v.label)),
        h('p', { class: 'hooks-desc', text: t(v.description) }),
      ),
      h(
        'div',
        { class: 'hooks-file' },
        h('div', { class: 'hooks-file-label', text: t('settings.hooks.file') }),
        h('code', { class: 'path', text: info.settingsPath ?? t('settings.hooks.fileUnknown') }),
      ),
      actions,
    );
  };

  void ctx.api.hooksStatus().then((info) => {
    ctx.onHooksStatus(info.status);
    render(info);
  });

  const el = h(
    'div',
    {},
    card(null, h('div', { class: 'row row-block' }, body, result)),
    h('p', { class: 'section-note', text: t('settings.hooks.note') }),
  );
  return { el, update() {} };
}
