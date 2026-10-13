// Advanced: the three timing numbers, and buttons to try the cat out.
import type { AppSettings } from '../../settings';
import { button, card, row, stepper, h, type Control } from '../controls';
import type { Ctx, SectionView } from '../context';
import { NUMBER_FIELDS, fromDisplay, stepped, toDisplay, type NumberField } from '../viewModel';
import type { UiKey } from '../../i18n';

const ROWS: readonly { field: NumberField; title: UiKey; desc: UiKey }[] = [
  { field: NUMBER_FIELDS.yellIntervalMs, title: 'settings.advanced.yellInterval', desc: 'settings.advanced.yellInterval.desc' },
  { field: NUMBER_FIELDS.maxYellsPerWake, title: 'settings.advanced.maxYells', desc: 'settings.advanced.maxYells.desc' },
  { field: NUMBER_FIELDS.staleAfterMs, title: 'settings.advanced.stale', desc: 'settings.advanced.stale.desc' },
];

export function advancedSection(ctx: Ctx): SectionView {
  const { t } = ctx;
  let current: AppSettings = ctx.settings;
  const steppers: [NumberField, Control<number>][] = [];

  const timingRows = ROWS.map(({ field, title, desc }) => {
    const name = t(title);
    const control = stepper({
      focusKey: field.key,
      value: toDisplay(field, current),
      unit: t(field.unit),
      decreaseLabel: t('settings.decrease', { name }),
      increaseLabel: t('settings.increase', { name }),
      min: field.min,
      max: field.max,
      onCommit(input) {
        const value =
          typeof input === 'string' ? fromDisplay(field, input) : stepped(field, toDisplay(field, current), input.step) * field.scale;
        if (value !== null && value !== current[field.key]) ctx.patch({ [field.key]: value });
      },
    });
    steppers.push([field, control]);
    return row({ title: name, desc: t(desc), control: control.el });
  });

  const el = h(
    'div',
    {},
    card(t('settings.advanced.timing'), ...timingRows),
    card(
      t('settings.advanced.test'),
      row({
        title: t('settings.advanced.wakeTest'),
        desc: t('settings.advanced.wakeTest.desc'),
        control: button(t('settings.advanced.wakeTest.button'), 'secondary', 'wakeTest', () => void ctx.api.wakeTest()),
      }),
      row({
        title: t('settings.advanced.sleep'),
        desc: t('settings.advanced.sleep.desc'),
        control: button(t('settings.advanced.sleep.button'), 'secondary', 'sleep', () => void ctx.api.sleep()),
      }),
    ),
  );

  return {
    el,
    update(next) {
      current = next;
      for (const [field, control] of steppers) control.set(toDisplay(field, next));
    },
  };
}
