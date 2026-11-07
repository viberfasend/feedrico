// Small enhancements. The page is complete without them.
//   1. The clock: places the sun, writes the clock note, offers Paper and Dusk instead.
//   2. The window: v cycles its Layouts, the caption follows.
//   3. Feedrico's keys: j/k move between sections, s stars one (in memory only), a hint in the
//      corner suggests the next key, and one button turns all of it off (WCAG 2.1.4).
//   4. The honest footer: counts this page's requests with the Performance API.
(function () {
  'use strict';
  var root = document.documentElement;
  var light = window.feedricoLight;
  var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

  // --- 1. The clock --------------------------------------------------------------------------

  // The sun's path in the hero SVG: one cubic Bézier from the left horizon to the right.
  var P = [[40, 420], [100, -240], [540, -240], [600, 420]];
  function onPath(t) {
    var m = 1 - t;
    return [0, 1].map(function (i) {
      return m * m * m * P[0][i] + 3 * m * m * t * P[1][i] + 3 * m * t * t * P[2][i] + t * t * t * P[3][i];
    });
  }

  function clockTime(hour) {
    var d = new Date();
    d.setHours(Math.floor(hour), Math.round((hour % 1) * 60), 0, 0);
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  var PHASE_WORDS = {
    morning: 'so the page is in morning light',
    midday: 'so the page is in full daylight',
    golden: 'so the page is in golden hour',
    dusk: 'so the page has turned to dusk',
    night: 'so the page is reading by lamplight',
  };

  function placeSun() {
    var sun = document.querySelector('[data-sun]');
    if (!sun || !light) return;
    var h = light.hour;
    var x, y;
    if (light.phase === 'night') {
      x = 600; y = 403; // The light has settled on the horizon, like a lamp left on.
    } else {
      var t = Math.min(0.96, Math.max(0.04, (h - 5) / 16));
      var p = onPath(t);
      x = p[0]; y = p[1];
    }
    sun.setAttribute('transform', 'translate(' + x.toFixed(1) + ' ' + y.toFixed(1) + ')');
    var label = sun.querySelector('[data-sun-time]');
    // The time sits outside the arc, away from the thread.
    var leftHalf = x < 320;
    label.setAttribute('x', leftHalf ? '-26' : '26');
    label.setAttribute('y', light.phase === 'night' ? '-30' : '-26');
    label.setAttribute('text-anchor', leftHalf ? 'end' : 'start');
    label.textContent = clockTime(h);
  }

  function writeNote() {
    var note = document.querySelector('[data-clock-note]');
    if (!note || !light) return;
    var time = clockTime(light.hour);
    var osDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    var text;
    if (light.choice === 'paper') {
      text = 'Paper, as you asked. Nothing is stored: next time, the page follows your clock again.';
    } else if (light.choice === 'dusk') {
      text = 'Dusk, as you asked. Nothing is stored: next time, the page follows your clock again.';
    } else if (osDark && root.dataset.theme === 'dark' && /morning|midday|golden/.test(light.phase)) {
      text = 'It’s ' + time + ' where you are. Your system asks for dark pages, so the page stays at dusk and only the sun moves. We only asked your clock.';
    } else {
      text = 'It’s ' + time + ' where you are, ' + PHASE_WORDS[light.phase] + '. We only asked your clock.';
    }
    note.textContent = text;
  }

  function syncThemeColor() {
    var bg = getComputedStyle(document.body).backgroundColor;
    document.querySelectorAll('meta[name="theme-color"]').forEach(function (m) {
      m.setAttribute('content', bg);
    });
  }

  function setupLight() {
    var control = document.querySelector('[data-light]');
    if (!control || !light) return;
    control.hidden = false;
    var current = control.querySelector('input[value="' + light.choice + '"]');
    if (current) current.checked = true;
    control.addEventListener('change', function (e) {
      light.choice = e.target.value;
      root.dataset.theme = light.theme(light.choice);
      writeNote();
      syncThemeColor();
    });
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function () {
      root.dataset.theme = light.theme(light.choice);
      writeNote();
      syncThemeColor();
    });
  }

  // --- 2. The window's Layouts ---------------------------------------------------------------

  var layoutInputs = Array.prototype.slice.call(document.querySelectorAll('input[name="layout"]'));
  var layoutName = document.querySelector('[data-layout-name]');
  var layoutsSeen = { list: true };

  function onLayoutChange() {
    var checked = layoutInputs.filter(function (i) { return i.checked; })[0];
    if (!checked) return;
    layoutsSeen[checked.value] = true;
    if (layoutName) layoutName.textContent = checked.parentNode.textContent.trim();
    updateHint();
  }
  layoutInputs.forEach(function (i) { i.addEventListener('change', onLayoutChange); });

  // ?layout=cards opens the window in that Layout (handy for links and testing).
  var askedLayout = new URLSearchParams(location.search).get('layout');
  layoutInputs.forEach(function (i) { if (i.value === askedLayout) i.checked = true; });

  function nextLayout() {
    var index = layoutInputs.findIndex(function (i) { return i.checked; });
    var next = layoutInputs[(index + 1) % layoutInputs.length];
    next.checked = true;
    onLayoutChange();
  }

  // --- 3. Feedrico's keys --------------------------------------------------------------------

  var sections = Array.prototype.slice.call(document.querySelectorAll('[data-section]'));
  var hint = document.querySelector('[data-keyhint]');
  var hintText = document.querySelector('[data-keyhint-text]');
  var toggle = document.querySelector('[data-keyhint-toggle]');
  var enabled = true;
  var used = {};
  var starred = 0;

  function currentIndex() {
    var line = window.innerHeight * 0.35;
    var index = 0;
    sections.forEach(function (s, i) {
      if (s.getBoundingClientRect().top <= line) index = i;
    });
    return index;
  }

  function goTo(index) {
    var section = sections[Math.max(0, Math.min(sections.length - 1, index))];
    section.scrollIntoView({ behavior: reduceMotion.matches ? 'auto' : 'smooth', block: 'start' });
    var heading = section.querySelector('h1[tabindex], h2[tabindex]');
    if (heading) heading.focus({ preventScroll: true });
  }

  var STAR_SVG =
    '<svg viewBox="0 0 20 20" aria-hidden="true" focusable="false"><path d="M10 1.8l2.5 5.2 5.7.8-4.1 4 1 5.7-5.1-2.7-5.1 2.7 1-5.7-4.1-4 5.7-.8z"/></svg>';

  function toggleStar(section) {
    var heading = section.querySelector('h1, h2');
    if (!heading) return;
    var star = heading.querySelector('.section-star');
    if (star) {
      star.remove();
      starred--;
      return 'Unstarred';
    }
    star = document.createElement('span');
    star.className = 'section-star';
    star.innerHTML = STAR_SVG + '<span class="visually-hidden"> (Starred)</span>';
    heading.appendChild(star);
    starred++;
    return 'Starred';
  }

  function say(html) {
    if (hintText) hintText.innerHTML = html;
  }

  function updateHint() {
    if (!hint || !enabled) return;
    var section = sections[currentIndex()];
    if (section && section.id === 'window' && Object.keys(layoutsSeen).length < 4) {
      say('<kbd>v</kbd> next Layout');
    } else if (!used.j) {
      say('<kbd>j</kbd> next section');
    } else if (!used.s) {
      say('<kbd>s</kbd> star this section');
    } else if (!used.k) {
      say('<kbd>k</kbd> previous section');
    } else {
      say('<kbd>j</kbd> <kbd>k</kbd> sections, <kbd>v</kbd> Layout, <kbd>s</kbd> star');
    }
  }

  function typing(target) {
    if (!target || target.isContentEditable) return true;
    var tag = target.tagName;
    if (tag === 'TEXTAREA' || tag === 'SELECT') return true;
    return tag === 'INPUT' && !/^(radio|checkbox|button|submit)$/.test(target.type);
  }

  document.addEventListener('keydown', function (e) {
    if (!enabled || e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey || typing(e.target)) return;
    var key = e.key;
    if (key === 'j' || key === 'k') {
      used[key] = true;
      goTo(currentIndex() + (key === 'j' ? 1 : -1));
    } else if (key === 'v') {
      used.v = true;
      nextLayout();
      var app = document.getElementById('window');
      if (sections[currentIndex()] !== app) goTo(sections.indexOf(app));
    } else if (key === 's') {
      used.s = true;
      var section = sections[currentIndex()];
      var verb = toggleStar(section);
      if (verb) {
        var title = (section.querySelector('h1, h2').firstChild.textContent || '').trim();
        say(verb + ' “' + title + '” <span class="keyhint__aside">in this tab only</span>');
        return;
      }
    } else {
      return;
    }
    e.preventDefault();
    window.setTimeout(updateHint, 600);
  });

  function setupKeys() {
    if (!hint) return;
    hint.hidden = false;
    document.querySelectorAll('[data-js-only]').forEach(function (el) { el.hidden = false; });
    toggle.addEventListener('click', function () {
      enabled = !enabled;
      toggle.setAttribute('aria-pressed', String(enabled));
      toggle.textContent = enabled ? 'Shortcuts on' : 'Shortcuts off';
      hint.classList.toggle('is-off', !enabled);
      if (enabled) updateHint();
      else say('Page shortcuts are off');
    });
    var pending = false;
    window.addEventListener('scroll', function () {
      if (pending) return;
      pending = true;
      window.requestAnimationFrame(function () {
        pending = false;
        updateHint();
      });
    }, { passive: true });
    updateHint();
  }

  // --- 4. The honest footer ------------------------------------------------------------------

  function countRequests() {
    var out = document.querySelector('[data-requests]');
    if (!out || !window.performance || !performance.getEntriesByType) return;
    var resources = performance.getEntriesByType('resource');
    var total = 1 + resources.length; // the page itself, then everything it loaded
    var elsewhere = resources.filter(function (r) {
      try { return new URL(r.name).origin !== location.origin; } catch (_) { return true; }
    }).length;
    out.textContent = elsewhere === 0
      ? 'This page made ' + total + ' requests, all to this site. Feedrico only talks to your Feeds.'
      : 'This page made ' + total + ' requests, and ' + elsewhere + ' went to another site. That is a bug; please tell us on GitHub.';
  }

  // --- Start ---------------------------------------------------------------------------------

  placeSun();
  writeNote();
  setupLight();
  syncThemeColor();
  setupKeys();
  onLayoutChange();

  if (document.readyState === 'complete') countRequests();
  else window.addEventListener('load', countRequests);
  if (window.PerformanceObserver) {
    try {
      new PerformanceObserver(countRequests).observe({ type: 'resource', buffered: false });
    } catch (_) { /* older browsers: the count from load stands */ }
  }
})();
