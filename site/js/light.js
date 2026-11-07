// The light follows your clock. Runs before the first paint so the page never flashes.
// It reads the time from your browser and nothing else: no location, no request, nothing stored.
// Testing: ?hour=19 (or 7.5) sets the hour; ?light=paper or ?light=dusk picks a theme.
(function () {
  'use strict';
  var root = document.documentElement;
  var params = new URLSearchParams(location.search);
  var now = new Date();
  var hour = now.getHours() + now.getMinutes() / 60;
  var override = parseFloat(params.get('hour'));
  if (override >= 0 && override < 24) hour = override;

  var phase =
    hour < 5 || hour >= 22 ? 'night'
    : hour < 10 ? 'morning'
    : hour < 16 ? 'midday'
    : hour < 19.5 ? 'golden'
    : 'dusk';

  var light = params.get('light');
  if (light !== 'paper' && light !== 'dusk') light = 'clock';

  // An explicit dark preference always wins: people who asked their system for dark pages never
  // get paper at noon. Otherwise the clock decides, and the page turns to dusk after sunset.
  function theme(choice) {
    if (choice === 'paper') return 'light';
    if (choice === 'dusk') return 'dark';
    var osDark = window.matchMedia && matchMedia('(prefers-color-scheme: dark)').matches;
    return osDark || phase === 'dusk' || phase === 'night' ? 'dark' : 'light';
  }

  root.dataset.phase = phase;
  root.dataset.theme = theme(light);
  root.classList.add('js');

  window.feedricoLight = { hour: hour, phase: phase, choice: light, theme: theme };
})();
