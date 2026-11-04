// Visual extension of the stock Tauri NSIS installer. Installation, elevation,
// WebView2, upgrades, shortcuts and removal remain owned by Tauri's template.
#include <windows.h>
#include <commctrl.h>
#include <dwmapi.h>
#include <objidl.h>
#include <gdiplus.h>
#include <cwchar>

namespace {
#include "midnight-colors.h"
constexpr UINT_PTR subclassId = 0x4D4944;
constexpr UINT_PTR refreshTimer = 0x4D4945;
constexpr wchar_t themedProperty[] = L"Midnight.Themed";
HBRUSH backgroundBrush = nullptr;
HBRUSH deepBrush = nullptr;
ULONG_PTR gdiplusToken = 0;
HWND installerWindow = nullptr;
HHOOK messageHook = nullptr;
bool applyingTheme = false;

int Scale(HWND window, int value) {
  return MulDiv(value, static_cast<int>(GetDpiForWindow(window)), 96);
}

Gdiplus::Color Color(COLORREF color) {
  return Gdiplus::Color(255, GetRValue(color), GetGValue(color), GetBValue(color));
}

void RoundRect(HDC dc, const RECT& bounds, int radius, COLORREF fill, COLORREF stroke, bool filled = true) {
  Gdiplus::Graphics graphics(dc);
  graphics.SetSmoothingMode(Gdiplus::SmoothingModeAntiAlias);
  const float x = static_cast<float>(bounds.left) + 0.75f;
  const float y = static_cast<float>(bounds.top) + 0.75f;
  const float width = static_cast<float>(bounds.right - bounds.left) - 1.5f;
  const float height = static_cast<float>(bounds.bottom - bounds.top) - 1.5f;
  const float diameter = static_cast<float>(radius * 2);
  Gdiplus::GraphicsPath path;
  path.AddArc(x, y, diameter, diameter, 180, 90);
  path.AddArc(x + width - diameter, y, diameter, diameter, 270, 90);
  path.AddArc(x + width - diameter, y + height - diameter, diameter, diameter, 0, 90);
  path.AddArc(x, y + height - diameter, diameter, diameter, 90, 90);
  path.CloseFigure();
  Gdiplus::SolidBrush brush(Color(fill));
  Gdiplus::Pen pen(Color(stroke));
  if (filled) graphics.FillPath(&brush, &path);
  graphics.DrawPath(&pen, &path);
}

void Text(HDC dc, HWND control, RECT bounds, COLORREF color, UINT flags) {
  wchar_t text[2048] = {};
  GetWindowTextW(control, text, ARRAYSIZE(text));
  const auto font = reinterpret_cast<HFONT>(SendMessageW(control, WM_GETFONT, 0, 0));
  const auto previous = SelectObject(dc, font ? font : GetStockObject(DEFAULT_GUI_FONT));
  SetBkMode(dc, TRANSPARENT);
  SetTextColor(dc, color);
  DrawTextW(dc, text, -1, &bounds, flags);
  SelectObject(dc, previous);
}

bool IsChoice(HWND control) {
  const auto type = GetWindowLongPtrW(control, GWL_STYLE) & BS_TYPEMASK;
  return type == BS_CHECKBOX || type == BS_AUTOCHECKBOX || type == BS_3STATE ||
         type == BS_AUTO3STATE || type == BS_RADIOBUTTON || type == BS_AUTORADIOBUTTON;
}

void PaintChoice(HWND control, HDC dc) {
  RECT bounds;
  GetClientRect(control, &bounds);
  FillRect(dc, &bounds, backgroundBrush);
  const int size = Scale(control, 14);
  const int top = (bounds.bottom - size) / 2;
  RECT box = {0, top, size, top + size};
  const bool checked = SendMessageW(control, BM_GETCHECK, 0, 0) != BST_UNCHECKED;
  const bool enabled = IsWindowEnabled(control) != FALSE;
  const auto type = GetWindowLongPtrW(control, GWL_STYLE) & BS_TYPEMASK;
  const bool radio = type == BS_RADIOBUTTON || type == BS_AUTORADIOBUTTON;
  RoundRect(dc, box, radio ? size / 2 : Scale(control, 4), checked ? rose : deep, checked ? rose : border);
  if (checked) {
    Gdiplus::Graphics graphics(dc);
    graphics.SetSmoothingMode(Gdiplus::SmoothingModeAntiAlias);
    if (radio) {
      Gdiplus::SolidBrush dot(Color(background));
      const int inset = Scale(control, 4);
      graphics.FillEllipse(&dot, inset, top + inset, size - inset * 2, size - inset * 2);
    } else {
      Gdiplus::Pen tick(Color(background), static_cast<float>(Scale(control, 2)));
      graphics.DrawLine(&tick, Scale(control, 3), top + Scale(control, 7), Scale(control, 6), top + Scale(control, 10));
      graphics.DrawLine(&tick, Scale(control, 6), top + Scale(control, 10), Scale(control, 11), top + Scale(control, 4));
    }
  }
  RECT text = bounds;
  text.left = size + Scale(control, 8);
  Text(dc, control, text, enabled ? ink : muted, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
  if (GetFocus() == control && !(SendMessageW(control, WM_QUERYUISTATE, 0, 0) & UISF_HIDEFOCUS)) {
    InflateRect(&text, -1, -1);
    DrawFocusRect(dc, &text);
  }
}

void PaintButton(const DRAWITEMSTRUCT& item) {
  FillRect(item.hDC, &item.rcItem, backgroundBrush);
  const bool primary = item.CtlID == IDOK;
  const bool disabled = (item.itemState & ODS_DISABLED) != 0;
  const bool pressed = (item.itemState & ODS_SELECTED) != 0;
  COLORREF fill = primary ? rose : surface;
  if (pressed) fill = primary ? roseLight : border;
  if (disabled) fill = surface;
  RoundRect(item.hDC, item.rcItem, Scale(item.hwndItem, 9), fill, primary && !disabled ? fill : border);
  Text(item.hDC, item.hwndItem, item.rcItem, disabled ? muted : primary ? background : roseLight,
       DT_CENTER | DT_VCENTER | DT_SINGLELINE);
  if ((item.itemState & ODS_FOCUS) && !(item.itemState & ODS_NOFOCUSRECT)) {
    RECT focus = item.rcItem;
    InflateRect(&focus, -Scale(item.hwndItem, 4), -Scale(item.hwndItem, 4));
    DrawFocusRect(item.hDC, &focus);
  }
}

void PaintProgress(HWND control, HDC dc) {
  RECT bounds;
  GetClientRect(control, &bounds);
  FillRect(dc, &bounds, backgroundBrush);
  RoundRect(dc, bounds, Scale(control, 5), deep, border);
  PBRANGE range = {};
  SendMessageW(control, PBM_GETRANGE, FALSE, reinterpret_cast<LPARAM>(&range));
  const auto position = static_cast<int>(SendMessageW(control, PBM_GETPOS, 0, 0));
  if (range.iHigh > range.iLow && position > range.iLow) {
    RECT filled = bounds;
    InflateRect(&filled, -2, -2);
    const int available = filled.right - filled.left;
    int width = MulDiv(available, position - range.iLow, range.iHigh - range.iLow);
    if (width > available) width = available;
    filled.right = filled.left + width;
    if (width >= Scale(control, 10)) RoundRect(dc, filled, Scale(control, 4), rose, rose);
  }
}

LRESULT CALLBACK ChoiceProc(HWND window, UINT message, WPARAM wParam, LPARAM lParam, UINT_PTR, DWORD_PTR) {
  if (message == WM_PAINT) {
    PAINTSTRUCT paint;
    HDC dc = BeginPaint(window, &paint);
    PaintChoice(window, dc);
    EndPaint(window, &paint);
    return 0;
  }
  if (message == WM_PRINTCLIENT) {
    PaintChoice(window, reinterpret_cast<HDC>(wParam));
    return 0;
  }
  if (message == WM_ERASEBKGND) return 1;
  const auto result = DefSubclassProc(window, message, wParam, lParam);
  if (message == BM_SETCHECK || message == WM_SETFOCUS || message == WM_KILLFOCUS ||
      message == WM_ENABLE || message == WM_UPDATEUISTATE || message == WM_LBUTTONUP || message == WM_KEYUP) {
    InvalidateRect(window, nullptr, TRUE);
  }
  if (message == WM_NCDESTROY) RemoveWindowSubclass(window, ChoiceProc, subclassId);
  return result;
}

LRESULT CALLBACK ProgressProc(HWND window, UINT message, WPARAM wParam, LPARAM lParam, UINT_PTR, DWORD_PTR) {
  if (message == WM_PAINT) {
    PAINTSTRUCT paint;
    HDC dc = BeginPaint(window, &paint);
    PaintProgress(window, dc);
    EndPaint(window, &paint);
    return 0;
  }
  if (message == WM_ERASEBKGND) return 1;
  const auto result = DefSubclassProc(window, message, wParam, lParam);
  if (message == PBM_SETPOS || message == PBM_DELTAPOS || message == PBM_SETRANGE32 || message == PBM_SETRANGE) {
    InvalidateRect(window, nullptr, FALSE);
  }
  if (message == WM_NCDESTROY) RemoveWindowSubclass(window, ProgressProc, subclassId);
  return result;
}

LRESULT CALLBACK ButtonProc(HWND window, UINT message, WPARAM wParam, LPARAM lParam, UINT_PTR, DWORD_PTR) {
  // Dialog navigation changes the default button with BM_SETSTYLE on each page.
  // Retain owner drawing while leaving the dialog's default command ID intact.
  if (message == BM_SETSTYLE) wParam = (wParam & ~BS_TYPEMASK) | BS_OWNERDRAW;
  const auto result = DefSubclassProc(window, message, wParam, lParam);
  if (message == WM_NCDESTROY) RemoveWindowSubclass(window, ButtonProc, subclassId);
  return result;
}

void PaintGroup(HWND window, HDC dc) {
  RECT bounds;
  GetClientRect(window, &bounds);
  bounds.top += Scale(window, 7);
  RoundRect(dc, bounds, Scale(window, 7), background, border, false);
  wchar_t caption[256] = {};
  GetWindowTextW(window, caption, ARRAYSIZE(caption));
  const auto font = reinterpret_cast<HFONT>(SendMessageW(window, WM_GETFONT, 0, 0));
  const auto previous = SelectObject(dc, font ? font : GetStockObject(DEFAULT_GUI_FONT));
  SIZE extent = {};
  GetTextExtentPoint32W(dc, caption, static_cast<int>(wcslen(caption)), &extent);
  SelectObject(dc, previous);
  RECT text = {Scale(window, 10), 0, Scale(window, 18) + extent.cx, extent.cy};
  FillRect(dc, &text, backgroundBrush);
  Text(dc, window, text, muted, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
}

LRESULT CALLBACK GroupProc(HWND window, UINT message, WPARAM wParam, LPARAM lParam, UINT_PTR, DWORD_PTR) {
  if (message == WM_PAINT) {
    PAINTSTRUCT paint;
    HDC dc = BeginPaint(window, &paint);
    PaintGroup(window, dc);
    EndPaint(window, &paint);
    return 0;
  }
  if (message == WM_PRINTCLIENT) {
    PaintGroup(window, reinterpret_cast<HDC>(wParam));
    return 0;
  }
  if (message == WM_ERASEBKGND) return 1;
  if (message == WM_NCDESTROY) RemoveWindowSubclass(window, GroupProc, subclassId);
  return DefSubclassProc(window, message, wParam, lParam);
}

LRESULT CALLBACK SurfaceProc(HWND window, UINT message, WPARAM wParam, LPARAM lParam, UINT_PTR, DWORD_PTR) {
  switch (message) {
    case WM_ERASEBKGND: {
      RECT bounds;
      GetClientRect(window, &bounds);
      FillRect(reinterpret_cast<HDC>(wParam), &bounds, backgroundBrush);
      return 1;
    }
    case WM_CTLCOLORDLG:
      return reinterpret_cast<LRESULT>(backgroundBrush);
    case WM_CTLCOLORSTATIC:
    case WM_CTLCOLORBTN:
    case WM_CTLCOLOREDIT:
    case WM_CTLCOLORLISTBOX: {
      const auto dc = reinterpret_cast<HDC>(wParam);
      const bool edit = message == WM_CTLCOLOREDIT || message == WM_CTLCOLORLISTBOX;
      SetTextColor(dc, ink);
      SetBkColor(dc, edit ? deep : background);
      return reinterpret_cast<LRESULT>(edit ? deepBrush : backgroundBrush);
    }
    case WM_DRAWITEM: {
      const auto item = reinterpret_cast<DRAWITEMSTRUCT*>(lParam);
      if (item && item->CtlType == ODT_BUTTON) {
        PaintButton(*item);
        return TRUE;
      }
      break;
    }
    case WM_NCDESTROY:
      if (window == installerWindow) {
        if (messageHook) UnhookWindowsHookEx(messageHook);
        messageHook = nullptr;
        installerWindow = nullptr;
      }
      RemoveWindowSubclass(window, SurfaceProc, subclassId);
      break;
  }
  return DefSubclassProc(window, message, wParam, lParam);
}

BOOL CALLBACK ThemeControl(HWND control, LPARAM) {
  const int id = GetDlgCtrlID(control);
  if (id == 1035 || id == 1045) {
    // MUI shows these etched separators again when changing pages.
    ShowWindow(control, SW_HIDE);
    return TRUE;
  }
  wchar_t caption[80] = {};
  GetWindowTextW(control, caption, ARRAYSIZE(caption));
  if (wcsncmp(caption, L"Nullsoft Install System", 22) == 0) {
    ShowWindow(control, SW_HIDE);
    return TRUE;
  }
  if (GetPropW(control, themedProperty)) return TRUE;
  SetPropW(control, themedProperty, reinterpret_cast<HANDLE>(1));
  wchar_t className[64] = {};
  GetClassNameW(control, className, ARRAYSIZE(className));
  if (wcscmp(className, L"#32770") == 0) {
    SetWindowSubclass(control, SurfaceProc, subclassId, 0);
  } else if (wcscmp(className, L"Button") == 0) {
    if (IsChoice(control)) {
      SetWindowSubclass(control, ChoiceProc, subclassId, 0);
    } else {
      const auto type = GetWindowLongPtrW(control, GWL_STYLE) & BS_TYPEMASK;
      // Native group boxes contain the directory edit and Browse button as
      // siblings. Painting them as filled buttons would obscure those fields.
      if (type == BS_GROUPBOX) {
        SetWindowSubclass(control, GroupProc, subclassId, 0);
        InvalidateRect(control, nullptr, TRUE);
        return TRUE;
      }
      if (type != BS_PUSHBUTTON && type != BS_DEFPUSHBUTTON && type != BS_OWNERDRAW) return TRUE;
      const auto style = GetWindowLongPtrW(control, GWL_STYLE);
      SetWindowLongPtrW(control, GWL_STYLE, (style & ~BS_TYPEMASK) | BS_OWNERDRAW);
      SetWindowSubclass(control, ButtonProc, subclassId, 0);
    }
  } else if (wcscmp(className, PROGRESS_CLASSW) == 0) {
    SetWindowSubclass(control, ProgressProc, subclassId, 0);
    SendMessageW(control, PBM_SETBARCOLOR, 0, rose);
    SendMessageW(control, PBM_SETBKCOLOR, 0, deep);
  } else if (wcscmp(className, WC_LISTVIEWW) == 0) {
    ListView_SetBkColor(control, deep);
    ListView_SetTextBkColor(control, deep);
    ListView_SetTextColor(control, ink);
  } else if (wcscmp(className, L"Static") == 0) {
    const auto type = GetWindowLongPtrW(control, GWL_STYLE) & SS_TYPEMASK;
    if (type == SS_ETCHEDHORZ || type == SS_ETCHEDVERT || type == SS_ETCHEDFRAME) ShowWindow(control, SW_HIDE);
  }
  InvalidateRect(control, nullptr, TRUE);
  return TRUE;
}

void CALLBACK Refresh(HWND window, UINT, UINT_PTR, DWORD) {
  // New NSIS pages create native children after GUIINIT. Only new controls are
  // styled; native command handlers, check states and keyboard navigation stay intact.
  EnumChildWindows(window, ThemeControl, 0);
}

LRESULT CALLBACK ThemeMessages(int code, WPARAM wParam, LPARAM lParam) {
  if (code >= 0 && installerWindow && !applyingTheme) {
    const auto message = reinterpret_cast<CWPRETSTRUCT*>(lParam);
    if ((message->message == WM_SHOWWINDOW || message->message == WM_INITDIALOG || message->message == WM_SETTEXT) &&
        IsChild(installerWindow, message->hwnd)) {
      // Style new pages before their first paint. The timer is a fallback for
      // native controls created without a show or initialization message.
      applyingTheme = true;
      ThemeControl(message->hwnd, 0);
      applyingTheme = false;
    }
  }
  return CallNextHookEx(messageHook, code, wParam, lParam);
}
}  // namespace

extern "C" void __stdcall Apply(HWND window) {
  if (!IsWindow(window) || GetPropW(window, themedProperty)) return;
  // Subclasses must remain valid after System::Call returns to NSIS.
  HMODULE pinned = nullptr;
  GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                    reinterpret_cast<LPCWSTR>(&Apply), &pinned);
  if (!gdiplusToken) {
    Gdiplus::GdiplusStartupInput startup;
    if (Gdiplus::GdiplusStartup(&gdiplusToken, &startup, nullptr) != Gdiplus::Ok) return;
    backgroundBrush = CreateSolidBrush(background);
    deepBrush = CreateSolidBrush(deep);
  }
  SetPropW(window, themedProperty, reinterpret_cast<HANDLE>(1));
  installerWindow = window;
  SetWindowSubclass(window, SurfaceProc, subclassId, 0);
  messageHook = SetWindowsHookExW(WH_CALLWNDPROCRET, ThemeMessages, pinned, GetCurrentThreadId());
  BOOL dark = TRUE;
  DwmSetWindowAttribute(window, 20, &dark, sizeof(dark));
  DwmSetWindowAttribute(window, 35, &border, sizeof(border));
  DwmSetWindowAttribute(window, 36, &deep, sizeof(deep));
  DwmSetWindowAttribute(window, 37, &roseLight, sizeof(roseLight));
  Refresh(window, 0, 0, 0);
  SetTimer(window, refreshTimer, 100, Refresh);
  RedrawWindow(window, nullptr, nullptr, RDW_ERASE | RDW_INVALIDATE | RDW_ALLCHILDREN);
}
