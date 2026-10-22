// General: start at login, which character, how it behaves and how big it is, how to move it.
import type { DragPermission } from '../../bridge/tauriBridge';
import { CAT_SCALE, CHARACTERS, type CharacterId } from '../../settings';
import { banner, button, card, h, pictureChoice, row, slider, svg, toggle } from '../controls';
import type { Ctx, SectionView } from '../context';
import { failureText } from '../viewModel';

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
      .catch((e: unknown) =>
        startError.append(
          banner('error', failureText(ctx.lang, e, (error) => t('settings.general.startAtLogin.error', { error }))),
        ),
      )
      .finally(() => startAtLogin.setBusy(false));
  });
  const character = pictureChoice<CharacterId>(
    'character',
    t('settings.general.character'),
    CHARACTERS.map((c) => ({ value: c, label: t(`settings.character.${c}`), desc: t(`settings.character.${c}.desc`) })),
    s.character,
    (v) => ctx.patch({ character: v }),
  );
  // Drawn from the real rigs once the window is up; without WebGL the cards keep just their names.
  void import('../../cat/pictures')
    .then((m) => m.characterPictures())
    .then((pictures) => character.setPictures(pictures))
    .catch(() => {});
  const pause = toggle('pause', !s.notificationsEnabled, (on) => ctx.patch({ notificationsEnabled: !on }));
  const hide = toggle('hide', s.hideWhileSleeping, (on) => ctx.patch({ hideWhileSleeping: on }));
  const sfw = toggle('sfw', s.sfwMode, (on) => ctx.patch({ sfwMode: on }));
  const motion = toggle('reduceMotion', s.reduceMotion, (on) => ctx.patch({ reduceMotion: on }));
  const size = slider({
    focusKey: 'catScale',
    value: s.catScale,
    ...CAT_SCALE,
    format: (v) => `${v}%`,
    onPreview: (v) => void ctx.api.previewCatScale(v),
    onChange: (v) => ctx.patch({ catScale: v }),
  });

  // macOS only: the Space hook needs the Accessibility permission. Hidden where it is not needed.
  const permissionRow = h('div', { attrs: { hidden: '' } });
  const showPermission = (p: DragPermission) => {
    permissionRow.hidden = p === 'not-needed';
    if (p === 'not-needed') return;
    const control =
      p === 'granted'
        ? h('span', { class: 'pill pill-ok' }, h('span', { class: 'pill-dot', attrs: { 'aria-hidden': 'true' } }), t('settings.general.permission.granted'))
        : button(t('settings.general.permission.grant'), 'primary', 'grantPermission', () => void ctx.api.openDragPermission());
    permissionRow.replaceChildren(
      row({ title: t('settings.general.permission'), desc: t('settings.general.permission.desc'), control }),
    );
  };
  void ctx.api.dragPermission().then(showPermission);
  void ctx.api.onDragPermission(showPermission);

  const el = h(
    'div',
    {},
    card(
      t('settings.general.startup'),
      row({ title: t('settings.general.startAtLogin'), desc: t('settings.general.startAtLogin.desc'), control: startAtLogin.el, below: startError }),
    ),
    card(
      t('settings.general.cat'),
      row({ title: t('settings.general.character'), desc: t('settings.general.character.desc'), below: character.el }),
      row({ title: t('settings.general.pause'), desc: t('settings.general.pause.desc'), control: pause.el }),
      row({ title: t('settings.general.hide'), desc: t('settings.general.hide.desc'), control: hide.el }),
      row({ title: t('settings.general.sfw'), desc: t('settings.general.sfw.desc'), control: sfw.el }),
      row({ title: t('settings.general.reduceMotion'), desc: t('settings.general.reduceMotion.desc'), control: motion.el }),
      row({ title: t('settings.general.size'), desc: t('settings.general.size.desc'), control: size.el }),
    ),
    card(
      t('settings.general.move'),
      h('div', { class: 'row row-hint' }, svg(MOVE_ICON), h('p', { class: 'hint-text', text: t('settings.general.move.how') })),
      permissionRow,
    ),
  );

  return {
    el,
    update(next) {
      startAtLogin.set(next.startAtLogin);
      character.set(next.character);
      pause.set(!next.notificationsEnabled);
      hide.set(next.hideWhileSleeping);
      sfw.set(next.sfwMode);
      motion.set(next.reduceMotion);
      size.set(next.catScale);
    },
  };
}
