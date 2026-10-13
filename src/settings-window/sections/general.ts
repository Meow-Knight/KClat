// General: start at login, how the cat behaves, how to move it.
import { banner, card, h, row, svg, toggle } from '../controls';
import type { Ctx, SectionView } from '../context';

const MOVE_ICON = `<svg class="hint-icon" viewBox="0 0 20 20" aria-hidden="true">
  <rect x="2.5" y="11" width="15" height="5" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.4"/>
  <path d="M10 2.5v6M7.6 4.8 10 2.5l2.4 2.3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/>
</svg>`;

export function generalSection(ctx: Ctx): SectionView {
  const { t, settings: s } = ctx;
  const startError = h('div', { class: 'row-error-slot' });

  const startAtLogin = toggle('startAtLogin', s.startAtLogin, (on) => {
    startAtLogin.setBusy(true);
    startError.replaceChildren();
    ctx.api
      .setStartAtLogin(on)
      .catch((e: unknown) => startError.append(banner('error', t('settings.general.startAtLogin.error', { error: String(e) }))))
      .finally(() => startAtLogin.setBusy(false));
  });
  const pause = toggle('pause', !s.notificationsEnabled, (on) => ctx.patch({ notificationsEnabled: !on }));
  const hide = toggle('hide', s.hideWhileSleeping, (on) => ctx.patch({ hideWhileSleeping: on }));
  const sfw = toggle('sfw', s.sfwMode, (on) => ctx.patch({ sfwMode: on }));
  const motion = toggle('reduceMotion', s.reduceMotion, (on) => ctx.patch({ reduceMotion: on }));

  const el = h(
    'div',
    {},
    card(
      t('settings.general.startup'),
      row({ title: t('settings.general.startAtLogin'), desc: t('settings.general.startAtLogin.desc'), control: startAtLogin.el, below: startError }),
    ),
    card(
      t('settings.general.cat'),
      row({ title: t('settings.general.pause'), desc: t('settings.general.pause.desc'), control: pause.el }),
      row({ title: t('settings.general.hide'), desc: t('settings.general.hide.desc'), control: hide.el }),
      row({ title: t('settings.general.sfw'), desc: t('settings.general.sfw.desc'), control: sfw.el }),
      row({ title: t('settings.general.reduceMotion'), desc: t('settings.general.reduceMotion.desc'), control: motion.el }),
    ),
    card(
      t('settings.general.move'),
      h('div', { class: 'row row-hint' }, svg(MOVE_ICON), h('p', { class: 'hint-text', text: t('settings.general.move.how') })),
    ),
  );

  return {
    el,
    update(next) {
      startAtLogin.set(next.startAtLogin);
      pause.set(!next.notificationsEnabled);
      hide.set(next.hideWhileSleeping);
      sfw.set(next.sfwMode);
      motion.set(next.reduceMotion);
    },
  };
}
