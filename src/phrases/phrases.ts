// Phrase data from spec §7.3.

/** `ready`: the cat's reply when you arm Ready ("wake me when Claude is done"). */
export type PhraseTag = 'wake' | 'done' | 'permission' | 'input' | 'ignored' | 'general' | 'ready';
export type PhraseLang = 'en' | 'vi';

export interface Phrase {
  id: string;
  text: string;
  lang: PhraseLang;
  tags: PhraseTag[];
  explicit: boolean;
}

const p = (id: string, text: string, lang: PhraseLang, tag: PhraseTag, explicit = false): Phrase => ({
  id,
  text,
  lang,
  tags: [tag],
  explicit,
});

export const PHRASES: readonly Phrase[] = [
  p('wake-1', 'Wake up, B**CH!!!', 'en', 'wake', true),
  p('wake-sfw', 'Wake up, human!!!', 'en', 'wake'),
  p('wake-vi', 'Dậy dậy dậy, dầu nhớt đây!!!', 'vi', 'wake'),
  p('en-01', "Claude's done. Your turn, lazy a**!", 'en', 'done', true),
  p('en-02', 'HEY! The robot is waiting on YOU!', 'en', 'general'),
  p('en-03', 'Get your butt back here, human!', 'en', 'general'),
  p('en-04', 'I was dreaming about tuna, you sh*t!', 'en', 'general', true),
  p('en-05', 'Stop scrolling. Read the d*mn output!', 'en', 'done', true),
  p('en-06', 'Tick tock, meatbag!', 'en', 'ignored'),
  p('en-07', 'Approve it or your coffee hits the floor.', 'en', 'permission'),
  p('en-08', 'Human.exe has stopped responding.', 'en', 'ignored'),
  p('en-09', "Earth to human! Claude's waiting!", 'en', 'general'),
  p('en-10', 'The AI did the work. Go take the credit.', 'en', 'done'),
  p('en-11', "Y or N. It's not rocket science, d*mmit!", 'en', 'permission', true),
  p('en-12', 'Did you die? Blink twice.', 'en', 'ignored'),
  p('en-13', "I'm a cat, not your f*cking alarm clock.", 'en', 'general', true),
  p('en-14', 'Back to work, you overpaid button-pusher!', 'en', 'general'),
  p('en-15', 'Read. Approve. Let me sleep. In that order.', 'en', 'permission'),
  p('en-16', 'Last warning before I sit on your keyboard.', 'en', 'ignored'),
  p('en-17', "Holy sh*t, it's done. Go look!", 'en', 'done', true),
  p('en-18', 'You call this pair programming?!', 'en', 'general'),
  p('en-19', "Stop pretending you're in a meeting!", 'en', 'ignored'),
  p('en-20', 'Hssss! Terminal! NOW!', 'en', 'general'),
  p('en-21', 'Meow. Translation: get your a** over here.', 'en', 'general', true),
  p('en-22', 'Your terminal is lonely, jerk.', 'en', 'input'),
  p('en-23', 'Ship it or I pee on your keyboard.', 'en', 'done', true),
  p('en-24', "Who's the senior dev here? Not you. MOVE.", 'en', 'general'),
  p('en-25', 'My tail is twitching. Bad news for you.', 'en', 'ignored'),
  p('en-26', "Claude finished. You haven't even started.", 'en', 'done'),
  p('en-27', 'Feed me, then press Enter.', 'en', 'input'),
  p('vi-01', 'Dậy đi đồ lười!!!', 'vi', 'general'),
  p('vi-02', 'Claude xong rồi kìa, còn ngồi đó à?!', 'vi', 'done'),
  p('vi-03', 'Duyệt lẹ đi, tao còn ngủ tiếp!', 'vi', 'permission'),
  p('vi-04', 'Đang mơ cá ngừ thì bị gọi dậy. Cảm ơn nhé!', 'vi', 'general'),
  p('vi-05', 'Lướt Facebook ít thôi, vào code đi!', 'vi', 'ignored'),
  p('vi-06', 'Ê! Terminal đợi mày nãy giờ đó!', 'vi', 'input'),
  p('vi-07', 'Bấm Enter giùm cái, mỏi mắt lắm rồi!', 'vi', 'input'),
  p('vi-08', 'Không duyệt là tao gạt ly cà phê đấy!', 'vi', 'permission'),
  p('vi-09', 'Xong rồi đó, ra nhận công đi sếp!', 'vi', 'done'),
  p('vi-10', 'Còn sống thì nhấp chuột một cái coi!', 'vi', 'ignored'),
  p('vi-11', 'Mèo này không làm báo thức miễn phí đâu!', 'vi', 'general'),
  p('vi-12', 'Sắp deadline rồi mà còn ngồi đó à?!', 'vi', 'ignored'),
  p('ready-en-1', "Fine. I'll yell when Claude's done.", 'en', 'ready'),
  p('ready-en-2', 'Napping. Wake you when it finishes.', 'en', 'ready'),
  p('ready-vi-1', 'Ok, Claude xong tao gọi.', 'vi', 'ready'),
  p('ready-vi-2', 'Ngủ tiếp đây, xong tao réo.', 'vi', 'ready'),
];
